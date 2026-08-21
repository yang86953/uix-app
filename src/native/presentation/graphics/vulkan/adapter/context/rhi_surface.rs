//! Vulkan `GraphicsSurface`、GpuRecipeContext 与原生提交/呈现闭环。
//!
//! acquire、submit、present 的共享身份关系由 platform 事务验证；本模块只
//! 管理 swapchain image、semaphore、fence 与 queue 操作。

use ash::vk;

use crate::core::{Errc, Error, PresentCoherency, Result};
use crate::native::present::{
    GpuRecipeContext, GraphicsContextLifecycle, resize_native_rhi_surface,
};
use crate::platform::presentation::rhi::{
    GraphicsSurface, GraphicsSurfaceCapabilities, RhiExtent, RhiPresentTransaction,
    RhiSurfaceResizeTransaction, SubmissionHandle, SurfaceFrame, SurfaceToken,
};

use super::rhi_frame::{VulkanRhiTarget, VulkanTargetOwner};
use super::{
    VulkanAcquiredFrame, VulkanContext, VulkanSubmittedFrame, failed_submit_error, vk_err,
};

impl VulkanContext {
    // 在复用唯一 command buffer 前等待上一提交，并重置单帧原生资源。
    pub(super) fn ensure_rhi_frame_prepared(&mut self) -> Result<()> {
        if self.rhi_device.is_frame_recording() {
            return Err(invalid_state(
                "Vulkan RHI cannot prepare while frame commands are recording",
            ));
        }
        if self.rhi_device.is_frame_prepared() {
            return Ok(());
        }
        if self.submitted_frame.is_some() {
            return Err(invalid_state(
                "Vulkan RHI cannot prepare a new frame before present",
            ));
        }
        self.wait_for_frame_fence("vkWaitForFences before RHI frame")?;
        if !self.present_fences.enabled() {
            self.present_lifetime
                .complete_submission(&self.device, &self.swapchain_loader)?;
        }
        self.rhi_device
            .prepare_frame(&self.device, self.command_buffer)
    }

    // 解析当前 acquired swapchain image 为 begin_render_pass 的原生目标。
    pub(super) fn acquired_surface_target(&self) -> Result<VulkanRhiTarget> {
        let acquired = self
            .acquired_frame
            .as_ref()
            .ok_or_else(|| invalid_state("Vulkan RHI surface pass has no acquired image"))?;
        let image_slot = acquired.image_index as usize;
        let image = self
            .swapchain_images
            .get(image_slot)
            .copied()
            .ok_or_else(|| invalid_state("Vulkan RHI acquired image is outside the swapchain"))?;
        let view = self
            .swapchain_image_views
            .get(image_slot)
            .copied()
            .ok_or_else(|| invalid_state("Vulkan RHI acquired image has no view"))?;
        let old_layout = self
            .image_layouts
            .get(image_slot)
            .copied()
            .ok_or_else(|| invalid_state("Vulkan RHI acquired image has no tracked layout"))?;
        Ok(VulkanRhiTarget {
            owner: VulkanTargetOwner::Surface(image_slot),
            image,
            view,
            format: self.swapchain_format,
            extent: RhiExtent::new(self.extent.width, self.extent.height),
            old_layout,
            final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
        })
    }

    // 结束 command buffer 并提交；Surface 帧绑定 present semaphore，离屏帧直接提交。
    pub(super) fn submit_rhi_frame(&mut self) -> Result<SubmissionHandle> {
        self.rhi_device
            .finish_recording(&self.device, self.command_buffer)?;
        let acquired = self.acquired_frame;
        let wait_stages =
            [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::TRANSFER];
        let wait_semaphores = acquired
            .as_ref()
            .map_or(&[][..], |_| std::slice::from_ref(&self.image_available));
        let signal_semaphores = acquired.as_ref().map_or(&[][..], |frame| {
            std::slice::from_ref(&frame.render_finished)
        });
        let mut submit = vk::SubmitInfo::default()
            .command_buffers(std::slice::from_ref(&self.command_buffer))
            .signal_semaphores(signal_semaphores);
        if !wait_semaphores.is_empty() {
            submit = submit
                .wait_semaphores(wait_semaphores)
                .wait_dst_stage_mask(&wait_stages);
        }
        // SAFETY: frame fence 已在上一帧完成；本次 reset 后只由下面 queue submit 使用。
        unsafe {
            self.device
                .reset_fences(std::slice::from_ref(&self.frame_fence))
                .map_err(|error| vk_err("vkResetFences RHI submit", error))?;
        }
        // SAFETY: command buffer 已结束，所有 semaphore/fence 均存活且归属同一 device。
        if let Err(error) = unsafe {
            self.device
                .queue_submit(self.queue, std::slice::from_ref(&submit), self.frame_fence)
        } {
            let recovery = self.restore_signaled_frame_fence();
            return Err(failed_submit_error(error, recovery));
        }
        let submission = self.rhi_device.issue_submission()?;
        if let Some(frame) = acquired {
            self.acquired_frame = None;
            if frame.present_fence.is_none() {
                self.present_lifetime
                    .on_submission_queued(frame.release_count);
            }
            self.submitted_frame = Some(VulkanSubmittedFrame {
                image_index: frame.image_index,
                acquire_suboptimal: frame.acquire_suboptimal,
                render_finished: frame.render_finished,
                present_fence: frame.present_fence,
                submission,
            });
        }
        Ok(submission)
    }
}

impl GraphicsSurface for VulkanContext {
    fn surface_capabilities(&self) -> GraphicsSurfaceCapabilities {
        GraphicsSurfaceCapabilities {
            present_coherency: PresentCoherency::FullOnly,
            readback: false,
        }
    }

    fn token(&self) -> SurfaceToken {
        self.surface_lifecycle.token()
    }

    fn acquire(&mut self) -> Result<SurfaceFrame> {
        let owner = self.active_device()?;
        owner.ensure_healthy()?;
        if self.acquired_frame.is_some() || self.submitted_frame.is_some() {
            return Err(invalid_state(
                "Vulkan RHI surface already owns an acquired or submitted image",
            ));
        }
        self.ensure_rhi_frame_prepared()?;
        // SAFETY: swapchain、semaphore 与 device 存活，Surface owner 串行调用 acquire。
        let (image_index, acquire_suboptimal) = match unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available,
                vk::Fence::null(),
            )
        } {
            Ok(result) => result,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                return match self.recreate_after_surface_change(
                    "vkAcquireNextImageKHR RHI",
                    vk::Result::ERROR_OUT_OF_DATE_KHR,
                    crate::platform::presentation::rhi::RhiSurfaceRecreateReason::AcquisitionRejected,
                ) {
                    Err(error) => Err(error),
                    Ok(()) => Err(invalid_state(
                        "Vulkan RHI surface recreation returned no retry error",
                    )),
                };
            }
            Err(error) => return Err(vk_err("vkAcquireNextImageKHR RHI", error)),
        };
        let image_slot = image_index as usize;
        let render_finished = self
            .render_finished
            .get(image_slot)
            .copied()
            .ok_or_else(|| invalid_state("Vulkan RHI acquired image has no present semaphore"))?;
        let release_count = if self.present_fences.enabled() {
            0
        } else {
            self.present_lifetime
                .release_count_for_acquire(image_slot)?
        };
        let present_fence = self
            .present_fences
            .prepare_for_present(&self.device, image_slot)?;
        self.acquired_frame = Some(VulkanAcquiredFrame {
            image_index,
            acquire_suboptimal,
            render_finished,
            present_fence,
            release_count,
        });
        Ok(SurfaceFrame::new(self.token()))
    }

    fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken> {
        let owner = self.active_device()?;
        owner.ensure_healthy()?;
        if self.acquired_frame.is_some() || self.submitted_frame.is_some() {
            return Err(invalid_state(
                "Vulkan RHI cannot resize with an acquired or submitted image",
            ));
        }
        let resize = RhiSurfaceResizeTransaction::validate(extent, self.token())?;
        if resize.extent() != self.token().extent {
            let dpr = self.width.max(1) as f32 / self.logical_width.max(1) as f32;
            self.recreate_swapchain(
                vk::Extent2D {
                    width: resize.extent().width,
                    height: resize.extent().height,
                },
                crate::platform::presentation::rhi::RhiSurfaceRecreateReason::Resize,
            )?;
            self.width = self.extent.width as i32;
            self.height = self.extent.height as i32;
            self.logical_width = (self.width as f32 / dpr.max(0.0001)).round().max(1.0) as i32;
            self.logical_height = (self.height as f32 / dpr.max(0.0001)).round().max(1.0) as i32;
            self.cpu_shadow.clear();
        }
        resize.complete(self.token())
    }

    fn present(&mut self, transaction: RhiPresentTransaction) -> Result<()> {
        let owner = self.active_device()?;
        owner.ensure_healthy()?;
        let current_token = self.token();
        let coherency = self.surface_capabilities().present_coherency;
        self.rhi_device
            .validate_present(transaction, current_token, coherency)?;
        let submitted = self
            .submitted_frame
            .as_ref()
            .ok_or_else(|| invalid_state("Vulkan RHI present has no submitted image"))?;
        if submitted.submission
            != self
                .rhi_device
                .validate_latest_submission(submitted.submission)?
        {
            return Err(invalid_state(
                "Vulkan RHI submitted image identity is inconsistent",
            ));
        }
        let submitted = self.submitted_frame.take().ok_or_else(|| {
            invalid_state("Vulkan RHI submitted image disappeared before present")
        })?;
        let swapchains = [self.swapchain];
        let indices = [submitted.image_index];
        let semaphores = [submitted.render_finished];
        let present_result = if let Some(present_fence) = submitted.present_fence {
            let fences = [present_fence];
            let mut fence_info = vk::SwapchainPresentFenceInfoEXT::default().fences(&fences);
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(&semaphores)
                .swapchains(&swapchains)
                .image_indices(&indices)
                .push_next(&mut fence_info);
            // SAFETY: present 只引用当前提交绑定的同步对象与 swapchain image。
            unsafe { self.swapchain_loader.queue_present(self.queue, &present) }
        } else {
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(&semaphores)
                .swapchains(&swapchains)
                .image_indices(&indices);
            // SAFETY: present 只引用当前提交绑定的同步对象与 swapchain image。
            unsafe { self.swapchain_loader.queue_present(self.queue, &present) }
        };
        match present_result {
            Ok(present_suboptimal) => {
                let image_slot = submitted.image_index as usize;
                if submitted.present_fence.is_some() {
                    self.present_fences.mark_submitted(image_slot)?;
                } else {
                    self.present_lifetime.mark_presented(image_slot)?;
                }
                if submitted.acquire_suboptimal || present_suboptimal {
                    self.recreate_after_surface_change(
                        "vkQueuePresentKHR RHI",
                        vk::Result::SUBOPTIMAL_KHR,
                        crate::platform::presentation::rhi::RhiSurfaceRecreateReason::PresentedNeedsRecreate,
                    )
                } else {
                    Ok(())
                }
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.recreate_after_surface_change(
                "vkQueuePresentKHR RHI",
                vk::Result::ERROR_OUT_OF_DATE_KHR,
                crate::platform::presentation::rhi::RhiSurfaceRecreateReason::PresentationRejected,
            ),
            Err(vk::Result::SUBOPTIMAL_KHR) => {
                // SUBOPTIMAL 已接受本次 present；先登记同步对象，再重建后续代际。
                let image_slot = submitted.image_index as usize;
                if submitted.present_fence.is_some() {
                    self.present_fences.mark_submitted(image_slot)?;
                } else {
                    self.present_lifetime.mark_presented(image_slot)?;
                }
                self.recreate_after_surface_change(
                    "vkQueuePresentKHR RHI",
                    vk::Result::SUBOPTIMAL_KHR,
                    crate::platform::presentation::rhi::RhiSurfaceRecreateReason::PresentedNeedsRecreate,
                )
            }
            Err(error) => Err(vk_err("vkQueuePresentKHR RHI", error)),
        }
    }
}

impl GpuRecipeContext for VulkanContext {
    fn rhi_context(
        &mut self,
    ) -> Result<&mut dyn crate::platform::presentation::rhi::GraphicsContextRhi> {
        self.active_device()?.ensure_healthy()?;
        Ok(self)
    }

    fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
        let present_surface = GraphicsContextLifecycle::present_surface(self);
        resize_native_rhi_surface(self, present_surface, width, height)
    }
}

fn invalid_state(message: &'static str) -> Error {
    Error::new(Errc::InvalidState, message)
}
