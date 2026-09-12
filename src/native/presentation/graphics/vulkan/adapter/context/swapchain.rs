//! Vulkan swapchain 逐 image 簿记与同步对象所有权。

use ash::vk;

use crate::core::{Errc, Error, Result};

use super::vk_err;

struct PresentFence {
    handle: vk::Fence,
    in_flight: bool,
}

pub(super) struct PresentFenceSet {
    entries: Vec<PresentFence>,
}

impl PresentFenceSet {
    pub(super) const fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub(super) fn create(device: &ash::Device, image_count: usize, enabled: bool) -> Result<Self> {
        if !enabled {
            return Ok(Self::empty());
        }
        let mut entries = Vec::new();
        entries.try_reserve_exact(image_count).map_err(|error| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                format!(
                    "VulkanContext: present fence allocation for {image_count} images failed: {error}"
                ),
            )
        })?;
        let create_info = vk::FenceCreateInfo::default();
        for image_index in 0..image_count {
            // SAFETY: device 存活；create_info 不含调用后保留的指针。
            match unsafe { device.create_fence(&create_info, None) } {
                Ok(handle) => entries.push(PresentFence {
                    handle,
                    in_flight: false,
                }),
                Err(error) => {
                    let mut fences = Self { entries };
                    fences.destroy(device);
                    return Err(vk_err(
                        &format!("vkCreateFence present[{image_index}]"),
                        error,
                    ));
                }
            }
        }
        Ok(Self { entries })
    }

    pub(super) fn enabled(&self) -> bool {
        !self.entries.is_empty()
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn prepare_for_present(
        &mut self,
        device: &ash::Device,
        image_slot: usize,
    ) -> Result<Option<vk::Fence>> {
        if self.entries.is_empty() {
            return Ok(None);
        }
        let entry = self.entries.get_mut(image_slot).ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                format!(
                    "VulkanContext: acquired swapchain image {image_slot} without a present fence"
                ),
            )
        })?;
        if entry.in_flight {
            // SAFETY: fence 属于 device；in_flight 只在 fence 随成功 present 提交后置位。
            unsafe {
                device
                    .wait_for_fences(std::slice::from_ref(&entry.handle), true, u64::MAX)
                    .map_err(|error| {
                        vk_err(&format!("vkWaitForFences present[{image_slot}]"), error)
                    })?;
                device
                    .reset_fences(std::slice::from_ref(&entry.handle))
                    .map_err(|error| {
                        vk_err(&format!("vkResetFences present[{image_slot}]"), error)
                    })?;
            }
            entry.in_flight = false;
        }
        Ok(Some(entry.handle))
    }

    pub(super) fn mark_submitted(&mut self, image_slot: usize) -> Result<()> {
        if self.entries.is_empty() {
            return Ok(());
        }
        let entry = self.entries.get_mut(image_slot).ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                format!(
                    "VulkanContext: presented swapchain image {image_slot} without a present fence"
                ),
            )
        })?;
        if entry.in_flight {
            return Err(Error::new(
                Errc::InvalidState,
                format!(
                    "VulkanContext: present fence for swapchain image {image_slot} is already in flight"
                ),
            ));
        }
        entry.in_flight = true;
        Ok(())
    }

    pub(super) fn wait_and_reset_all(&mut self, device: &ash::Device) -> Result<()> {
        for (image_slot, entry) in self.entries.iter_mut().enumerate() {
            if !entry.in_flight {
                continue;
            }
            // SAFETY: fence 属于 device，且只等待成功提交给 presentation engine 的 fence。
            unsafe {
                device
                    .wait_for_fences(std::slice::from_ref(&entry.handle), true, u64::MAX)
                    .map_err(|error| {
                        vk_err(&format!("vkWaitForFences present[{image_slot}]"), error)
                    })?;
                device
                    .reset_fences(std::slice::from_ref(&entry.handle))
                    .map_err(|error| {
                        vk_err(&format!("vkResetFences present[{image_slot}]"), error)
                    })?;
            }
            entry.in_flight = false;
        }
        Ok(())
    }

    pub(super) fn destroy(&mut self, device: &ash::Device) {
        for entry in self.entries.drain(..) {
            // SAFETY: fence 由同一 device 创建；调用方已等待完成或正在处理 device lost。
            unsafe { device.destroy_fence(entry.handle, None) };
        }
    }
}

pub(crate) struct PresentCompletion {
    presented_images: Vec<bool>,
    release_after_submission: usize,
}

impl PresentCompletion {
    fn empty() -> Self {
        Self {
            presented_images: Vec::new(),
            release_after_submission: 0,
        }
    }

    pub(crate) fn replace_generation(&mut self, presented_images: Vec<bool>) {
        self.presented_images = presented_images;
    }

    pub(crate) fn release_count_for_acquire(
        &self,
        image_slot: usize,
        retired_count: usize,
    ) -> Result<usize> {
        let was_presented = self.presented_images.get(image_slot).ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                format!(
                    "VulkanContext: acquired swapchain image {image_slot} without presentation history"
                ),
            )
        })?;
        Ok(if *was_presented { retired_count } else { 0 })
    }

    pub(crate) fn mark_presented(&mut self, image_slot: usize) -> Result<()> {
        let presented = self.presented_images.get_mut(image_slot).ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                format!(
                    "VulkanContext: presented swapchain image {image_slot} without presentation history"
                ),
            )
        })?;
        *presented = true;
        Ok(())
    }

    pub(crate) fn on_submission_queued(&mut self, release_count: usize) {
        self.release_after_submission = self.release_after_submission.max(release_count);
    }

    pub(crate) fn completed_submission_count(&mut self) -> usize {
        std::mem::take(&mut self.release_after_submission)
    }

    fn pending_release_count(&self) -> usize {
        self.release_after_submission
    }
}


struct RetiredSwapchain {
    handle: vk::SwapchainKHR,
    render_finished: Vec<vk::Semaphore>,
}

pub(super) struct PresentLifetime {
    completion: PresentCompletion,
    retired: Vec<RetiredSwapchain>,
}

impl PresentLifetime {
    pub(super) fn new() -> Self {
        Self {
            completion: PresentCompletion::empty(),
            retired: Vec::new(),
        }
    }

    pub(super) fn reserve_retirement(&mut self) -> Result<()> {
        self.retired.try_reserve(1).map_err(|error| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                format!("VulkanContext: retired swapchain allocation failed: {error}"),
            )
        })
    }

    pub(super) fn begin_generation(&mut self, presented_images: Vec<bool>) {
        self.completion.replace_generation(presented_images);
    }

    pub(super) fn retire_reserved(
        &mut self,
        handle: vk::SwapchainKHR,
        render_finished: Vec<vk::Semaphore>,
    ) {
        // reserve_retirement 紧邻本调用且 context 线程绑定；push 不会再次分配。
        self.retired.push(RetiredSwapchain {
            handle,
            render_finished,
        });
    }

    pub(super) fn release_count_for_acquire(&self, image_slot: usize) -> Result<usize> {
        self.completion
            .release_count_for_acquire(image_slot, self.retired.len())
    }

    pub(super) fn on_submission_queued(&mut self, release_count: usize) {
        self.completion.on_submission_queued(release_count);
    }

    pub(super) fn mark_presented(&mut self, image_slot: usize) -> Result<()> {
        self.completion.mark_presented(image_slot)
    }

    pub(super) fn complete_submission(
        &mut self,
        device: &ash::Device,
        loader: &ash::khr::swapchain::Device,
    ) -> Result<()> {
        let count = self.completion.pending_release_count();
        if count > self.retired.len() {
            return Err(Error::new(
                Errc::InvalidState,
                format!(
                    "VulkanContext: {count} retired swapchains became releasable but only {} remain",
                    self.retired.len()
                ),
            ));
        }
        let count = self.completion.completed_submission_count();
        for retired in self.retired.drain(..count) {
            destroy_retired_swapchain(device, loader, retired);
        }
        Ok(())
    }

    pub(super) fn destroy_all(
        &mut self,
        device: &ash::Device,
        loader: &ash::khr::swapchain::Device,
    ) {
        for retired in self.retired.drain(..) {
            destroy_retired_swapchain(device, loader, retired);
        }
        let _ = self.completion.completed_submission_count();
    }
}

pub(super) fn allocate_presented_images(image_count: usize) -> Result<Vec<bool>> {
    let mut presented = Vec::new();
    presented.try_reserve_exact(image_count).map_err(|error| {
        Error::new(
            Errc::GraphicsOutOfMemory,
            format!(
                "VulkanContext: presentation history allocation for {image_count} images failed: {error}"
            ),
        )
    })?;
    presented.resize(image_count, false);
    Ok(presented)
}

fn destroy_retired_swapchain(
    device: &ash::Device,
    loader: &ash::khr::swapchain::Device,
    mut retired: RetiredSwapchain,
) {
    // SAFETY: acquire-proof 已证明旧 presentation 完成，或调用方正处理 device lost/shutdown。
    unsafe { loader.destroy_swapchain(retired.handle, None) };
    destroy_semaphores(device, &mut retired.render_finished);
}

pub(crate) fn allocate_image_layouts(image_count: usize) -> Result<Vec<vk::ImageLayout>> {
    let mut layouts = Vec::new();
    layouts.try_reserve_exact(image_count).map_err(|error| {
        Error::new(
            Errc::GraphicsOutOfMemory,
            format!(
                "VulkanContext: swapchain layout allocation for {image_count} images failed: {error}"
            ),
        )
    })?;
    layouts.resize(image_count, vk::ImageLayout::UNDEFINED);
    Ok(layouts)
}

// 为每个 swapchain image 创建唯一颜色 attachment view。
pub(super) fn create_swapchain_image_views(
    device: &ash::Device,
    images: &[vk::Image],
    format: vk::Format,
) -> Result<Vec<vk::ImageView>> {
    let mut views = Vec::new();
    views.try_reserve_exact(images.len()).map_err(|error| {
        Error::new(
            Errc::GraphicsOutOfMemory,
            format!(
                "VulkanContext: swapchain image-view allocation for {} images failed: {error}",
                images.len()
            ),
        )
    })?;
    for (image_index, image) in images.iter().copied().enumerate() {
        let create_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .components(vk::ComponentMapping::default())
            .subresource_range(super::super::rhi::color_subresource_range());
        // SAFETY: image 属于当前 swapchain/device，格式与 swapchain 创建格式一致。
        match unsafe { device.create_image_view(&create_info, None) } {
            Ok(view) => views.push(view),
            Err(error) => {
                destroy_image_views(device, &mut views);
                return Err(vk_err(
                    &format!("vkCreateImageView swapchain[{image_index}]"),
                    error,
                ));
            }
        }
    }
    Ok(views)
}

// 按创建逆序销毁 swapchain image views。
pub(super) fn destroy_image_views(device: &ash::Device, views: &mut Vec<vk::ImageView>) {
    // SAFETY: 每个 view 由当前 device 创建，调用方保证对应 framebuffer 已先释放。
    unsafe {
        for view in views.drain(..).rev() {
            device.destroy_image_view(view, None);
        }
    }
}

pub(super) fn create_render_finished_semaphores(
    device: &ash::Device,
    image_count: usize,
) -> Result<Vec<vk::Semaphore>> {
    let create_info = vk::SemaphoreCreateInfo::default();
    let mut semaphores = Vec::new();
    semaphores.try_reserve_exact(image_count).map_err(|error| {
        Error::new(
            Errc::GraphicsOutOfMemory,
            format!(
                "VulkanContext: present semaphore allocation for {image_count} images failed: {error}"
            ),
        )
    })?;
    for image_index in 0..image_count {
        // SAFETY: device 存活；create_info 不含调用后保留的指针。
        match unsafe { device.create_semaphore(&create_info, None) } {
            Ok(semaphore) => semaphores.push(semaphore),
            Err(error) => {
                destroy_semaphores(device, &mut semaphores);
                return Err(vk_err(
                    &format!("vkCreateSemaphore render_finished[{image_index}]"),
                    error,
                ));
            }
        }
    }
    Ok(semaphores)
}

pub(super) fn destroy_semaphores(device: &ash::Device, semaphores: &mut Vec<vk::Semaphore>) {
    for semaphore in semaphores.drain(..) {
        // SAFETY: semaphore 由同一 device 创建，且调用方已完成相应 teardown 等待。
        unsafe { device.destroy_semaphore(semaphore, None) };
    }
}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../../../../../tests-src/native/presentation/graphics/vulkan/adapter/context/swapchain_tests.rs"]
mod swapchain_tests;

// GPU 验证专用实现位于 tests-src（模块级 include! 保持原作用域与 cfg），
// 仅 cargo test（含 RUSTFLAGS parity 入口）构建读取，发布包不携带。
#[cfg(test)]
include!("../../../../../../../tests-src/native/presentation/graphics/vulkan/adapter/context/swapchain_parity_fns.rs");
