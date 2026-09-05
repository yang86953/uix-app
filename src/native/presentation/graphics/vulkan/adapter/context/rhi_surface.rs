//! Vulkan `GraphicsSurface`、GpuRecipeContext 与原生提交/呈现闭环。
//!
//! acquire、submit、present 的共享身份关系由 platform 事务验证；本模块只
//! 管理 swapchain image、semaphore、fence 与 queue 操作。

use ash::vk;

use crate::core::{Errc, Error, PresentCoherency, Result};
use crate::platform::presentation::rhi::{
    GraphicsSurface, GraphicsSurfaceCapabilities, RhiExtent, RhiPresentTransaction, RhiScissor,
    RhiSurfaceReadback, RhiSurfaceResizeTransaction, SubmissionHandle, SurfaceFrame, SurfaceToken,
};
use crate::platform::presentation::{
    GpuRecipeContext, GraphicsContextLifecycle, resize_native_rhi_surface,
};

use super::rhi_frame::{VulkanRhiTarget, VulkanTargetOwner};
use super::rhi_surface_readback::{record_surface_readback, supports_surface_readback_format};
use super::{
    VulkanAcquiredFrame, VulkanContext, VulkanSubmittedFrame, failed_submit_error, vk_err,
};

impl VulkanContext {
    // 为同一 FramePlan 的后续命令复用已录制中的 command buffer。
    pub(super) fn ensure_rhi_commands_ready(&mut self) -> Result<()> {
        if self.rhi_device.is_frame_recording() {
            return Ok(());
        }
        self.ensure_rhi_frame_prepared()
    }

    // 只允许显式 parity 组合根在无在途 frame 时安排一个原生 Surface 结果。
    #[cfg(feature = "vulkan-parity-test")]
    pub(crate) fn inject_surface_fault_for_parity_test(
        &mut self,
        fault: super::VulkanSurfaceFaultForParity,
    ) -> Result<()> {
        self.active_device()?;
        if self.acquired_frame.is_some() || self.submitted_frame.is_some() {
            return Err(invalid_state(
                "Vulkan Surface parity fault requires an idle frame boundary",
            ));
        }
        if self.surface_fault_for_parity.is_some() {
            return Err(invalid_state(
                "Vulkan Surface parity fault is already pending",
            ));
        }
        self.surface_fault_for_parity = Some(fault);
        Ok(())
    }

    // 只在对应原生调用边界消费一次匹配的 parity 故障。
    #[cfg(feature = "vulkan-parity-test")]
    fn take_surface_fault_for_parity_test(
        &mut self,
        fault: super::VulkanSurfaceFaultForParity,
    ) -> bool {
        if self.surface_fault_for_parity == Some(fault) {
            self.surface_fault_for_parity = None;
            true
        } else {
            false
        }
    }

    // 在复用唯一 command buffer 前等待上一提交，并重置单帧原生资源。
    pub(super) fn ensure_rhi_frame_prepared(&mut self) -> Result<()> {
        if self.rhi_device.is_frame_recording() {
            return Err(invalid_state(
                "Vulkan RHI cannot prepare while frame commands are recording",
            ));
        }
        if self.rhi_device.is_frame_prepared() && !self.rhi_device.has_texture_move_scratch() {
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
    pub(super) fn submit_rhi_frame(
        &mut self,
        owner: &super::super::device::VulkanDevice,
    ) -> Result<SubmissionHandle> {
        self.rhi_device
            .finish_recording(&self.device, self.command_buffer)?;
        let acquired = self.acquired_frame;
        // acquire semaphore 必须先于旧 PRESENT 布局转换及其后全部图形/传输命令生效。
        let wait_stages = [vk::PipelineStageFlags::ALL_COMMANDS];
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
        let submit_result = owner.with_queue("vkQueueSubmit RHI frame", |queue| unsafe {
            self.device
                .queue_submit(queue, std::slice::from_ref(&submit), self.frame_fence)
        })?;
        if let Err(error) = submit_result {
            // DEVICE_LOST 后不再创建原生同步对象，旧 fence 留给逆序 shutdown 回收。
            let recovery = if error == vk::Result::ERROR_DEVICE_LOST {
                Ok(())
            } else {
                self.restore_signaled_frame_fence()
            };
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
            // capability 直接投影本代真实 Surface usage 与已实现的格式规范化集合。
            readback: self
                .surface_supported_usage_flags
                .contains(vk::ImageUsageFlags::TRANSFER_SRC)
                && supports_surface_readback_format(self.swapchain_format),
        }
    }

    fn token(&self) -> SurfaceToken {
        self.surface_lifecycle.token()
    }

    #[cfg(target_os = "linux")]
    fn surface_corner_radius(&self) -> f32 {
        // SAFETY: native_surface 指向窗口 owner 在 VulkanContext 生命周期内保持稳定的 descriptor。
        unsafe {
            crate::native::presentation::graphics::platform::linux::WaylandSurfaceHandle::from_native(
                self.native_surface,
            )
        }
        .and_then(|surface| surface.metrics.snapshot())
        .map_or(0.0, |snapshot| snapshot.corner_radius as f32)
    }

    #[cfg(target_os = "linux")]
    fn surface_shadow_fill(&self) -> ([f32; 4], f32) {
        // SAFETY: native_surface 指向窗口 owner 在 VulkanContext 生命周期内保持稳定的 descriptor。
        unsafe {
            crate::native::presentation::graphics::platform::linux::WaylandSurfaceHandle::from_native(
                self.native_surface,
            )
        }
        .and_then(|surface| surface.metrics.snapshot())
        .map_or(([0.0; 4], 0.0), |snapshot| {
            (snapshot.shadow_fill_alphas, snapshot.shadow_fill_range as f32)
        })
    }

    fn acquire(&mut self) -> Result<SurfaceFrame> {
        let owner = self.active_device()?;
        if self.acquired_frame.is_some() || self.submitted_frame.is_some() {
            return Err(invalid_state(
                "Vulkan RHI surface already owns an acquired or submitted image",
            ));
        }
        owner.observe(self.ensure_rhi_frame_prepared())?;
        // OUT_OF_DATE 注入发生在真实 acquire 前，因此不会产生可被旧 token 提交的 image。
        #[cfg(feature = "vulkan-parity-test")]
        let reject_acquire_for_parity = self.take_surface_fault_for_parity_test(
            super::VulkanSurfaceFaultForParity::AcquireOutOfDate,
        );
        #[cfg(not(feature = "vulkan-parity-test"))]
        let reject_acquire_for_parity = false;
        // SAFETY: swapchain、semaphore 与 device 存活，Surface owner 串行调用 acquire。
        let acquire_result = if reject_acquire_for_parity {
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR)
        } else {
            // SAFETY: swapchain、semaphore 与 device 存活，Surface owner 串行调用 acquire。
            unsafe {
                self.swapchain_loader.acquire_next_image(
                    self.swapchain,
                    u64::MAX,
                    self.image_available,
                    vk::Fence::null(),
                )
            }
        };
        let (image_index, acquire_suboptimal) = match acquire_result {
            Ok(result) => result,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                return owner.observe(match self.recreate_after_surface_change(
                    "vkAcquireNextImageKHR RHI",
                    vk::Result::ERROR_OUT_OF_DATE_KHR,
                    crate::platform::presentation::rhi::RhiSurfaceRecreateReason::AcquisitionRejected,
                ) {
                    Err(error) => Err(error),
                    Ok(()) => Err(invalid_state(
                        "Vulkan RHI surface recreation returned no retry error",
                    )),
                });
            }
            Err(error) => return Err(owner.error("vkAcquireNextImageKHR RHI", error)),
        };
        // acquire SUBOPTIMAL 仍必须持有真实 WSI image，并由成功 present 后的共享语义重建。
        #[cfg(feature = "vulkan-parity-test")]
        let acquire_suboptimal = acquire_suboptimal
            || self.take_surface_fault_for_parity_test(
                super::VulkanSurfaceFaultForParity::AcquireSuboptimal,
            );
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
        let present_fence = owner.observe(
            self.present_fences
                .prepare_for_present(&self.device, image_slot),
        )?;
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
        if self.acquired_frame.is_some() || self.submitted_frame.is_some() {
            return Err(invalid_state(
                "Vulkan RHI cannot resize with an acquired or submitted image",
            ));
        }
        let resize = RhiSurfaceResizeTransaction::validate(extent, self.token())?;
        if resize.extent() != self.token().extent {
            let dpr = self.width.max(1) as f32 / self.logical_width.max(1) as f32;
            owner.observe(self.recreate_swapchain(
                vk::Extent2D {
                    width: resize.extent().width,
                    height: resize.extent().height,
                },
                crate::platform::presentation::rhi::RhiSurfaceRecreateReason::Resize,
            ))?;
            self.width = self.extent.width as i32;
            self.height = self.extent.height as i32;
            self.logical_width = (self.width as f32 / dpr.max(0.0001)).round().max(1.0) as i32;
            self.logical_height = (self.height as f32 / dpr.max(0.0001)).round().max(1.0) as i32;
            self.cpu_shadow.clear();
        }
        resize.complete(self.token())
    }

    // 回读已提交最终 composite、尚未 present 的当前 swapchain image。
    fn read_surface_pixels(&mut self, region: RhiScissor) -> Result<RhiSurfaceReadback> {
        let owner = self.active_device()?;
        if !self.surface_capabilities().readback {
            return Err(Error::new(
                Errc::NotImplemented,
                "Vulkan Surface readback requires TRANSFER_SRC and a supported swapchain format",
            ));
        }
        let token = self.token();
        // 复用 platform 唯一区域契约，禁止 Adapter 静默裁切。
        RhiSurfaceReadback::validate_region(region, token.extent)?;
        let submitted = self.submitted_frame.as_ref().ok_or_else(|| {
            invalid_state("Vulkan Surface readback requires a submitted image before present")
        })?;
        let image_slot = submitted.image_index as usize;
        let image = self
            .swapchain_images
            .get(image_slot)
            .copied()
            .ok_or_else(|| invalid_state("Vulkan Surface readback image is outside swapchain"))?;
        let layout =
            self.image_layouts.get(image_slot).copied().ok_or_else(|| {
                invalid_state("Vulkan Surface readback image has no tracked layout")
            })?;
        if layout != vk::ImageLayout::PRESENT_SRC_KHR {
            return Err(invalid_state(
                "Vulkan Surface readback image is not ready for before-present transfer",
            ));
        }
        let pixel_count = (region.width as usize)
            .checked_mul(region.height as usize)
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidArgument,
                    "Vulkan Surface readback pixel count overflows",
                )
            })?;
        let required = u64::try_from(pixel_count)
            .ok()
            .and_then(|count| count.checked_mul(4))
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidArgument,
                    "Vulkan Surface readback staging size overflows",
                )
            })?;
        let instance = self
            .runtime
            .as_ref()
            .ok_or_else(|| invalid_state("Vulkan Surface readback requested after shutdown"))?
            .instance()
            .clone();
        let staging = self.surface_readback.ensure_capacity(
            &instance,
            self.physical_device,
            &self.device,
            required,
        );
        owner.observe(staging)?;

        let destination = self.surface_readback.buffer();
        let queue_family_index = self.adapter_info.queue_family_index;
        let device = self.device.clone();
        // 同一共享 queue 上的即时提交排在最终 composite submit 之后，并等待 GPU idle；
        // render_finished 保持 signaled，随后仍由真实 present 唯一消费。
        let readback = owner.with_queue("vkQueueSubmit Surface readback", |queue| {
            self.rhi_device
                .execute_immediate(&device, queue, queue_family_index, |command| {
                    record_surface_readback(&device, command, image, destination, region);
                })
        })?;
        owner.observe(readback)?;
        let pixels =
            self.surface_readback
                .read_pixels(&self.device, self.swapchain_format, pixel_count);
        // immediate 已确认 GPU 完成，read_pixels 已解除映射且返回自有像素。
        // 连同读取失败一起回收临时 staging，避免截图永久抬高窗口的内存基线。
        self.surface_readback.release(&self.device);
        let pixels = owner.observe(pixels)?;
        RhiSurfaceReadback::try_new(region, token.extent, pixels)
    }

    fn present(&mut self, transaction: RhiPresentTransaction) -> Result<()> {
        let owner = self.active_device()?;
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
        // OUT_OF_DATE 注入发生在真实 present 前，旧 frame 只完成 GPU submit，不进入 WSI。
        #[cfg(feature = "vulkan-parity-test")]
        let reject_present_for_parity = self.take_surface_fault_for_parity_test(
            super::VulkanSurfaceFaultForParity::PresentOutOfDate,
        );
        #[cfg(not(feature = "vulkan-parity-test"))]
        let reject_present_for_parity = false;
        #[cfg(feature = "vulkan-parity-test")]
        if reject_present_for_parity {
            self.replace_present_sync_for_parity = true;
        }
        let present_result = if reject_present_for_parity {
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR)
        } else if let Some(present_fence) = submitted.present_fence {
            let fences = [present_fence];
            let mut fence_info = vk::SwapchainPresentFenceInfoEXT::default().fences(&fences);
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(&semaphores)
                .swapchains(&swapchains)
                .image_indices(&indices)
                .push_next(&mut fence_info);
            // SAFETY: present 只引用当前提交绑定的同步对象与 swapchain image。
            owner.with_queue("vkQueuePresentKHR RHI", |queue| unsafe {
                self.swapchain_loader.queue_present(queue, &present)
            })?
        } else {
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(&semaphores)
                .swapchains(&swapchains)
                .image_indices(&indices);
            // SAFETY: present 只引用当前提交绑定的同步对象与 swapchain image。
            owner.with_queue("vkQueuePresentKHR RHI", |queue| unsafe {
                self.swapchain_loader.queue_present(queue, &present)
            })?
        };
        // present SUBOPTIMAL 注入保留上面的真实 queue present 与同步所有权，只改写状态映射。
        #[cfg(feature = "vulkan-parity-test")]
        let present_result = if self.take_surface_fault_for_parity_test(
            super::VulkanSurfaceFaultForParity::PresentSuboptimal,
        ) {
            present_result.map(|_| true)
        } else {
            present_result
        };
        let result = match present_result {
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
        };
        owner.observe(result)
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
