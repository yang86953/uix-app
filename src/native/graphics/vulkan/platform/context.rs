//! Vulkan graphics context for Linux Wayland, Windows Win32, and macOS MoltenVK
//! (PixelUpload via shared swapchain stage/present).

use std::ffi::c_void;
use std::ptr;
use std::rc::Rc;

use ash::vk;

use crate::core::{Errc, Error, Result};
use crate::native::traits::present::{
    GraphicsBackend, GraphicsContextCaps, IGraphicsContext, PresentDamage,
};

#[cfg(target_os = "macos")]
use crate::native::backends::macos::platform as macos_surface;

use super::adapter::{select_queue, VulkanAdapterInfo};
pub(crate) use super::device::staging_size;
use super::device::{VulkanDevice, VulkanRuntime};
use super::drawable::drawable_size;
use super::surface::{
    choose_composite_alpha, choose_extent, choose_present_mode, choose_surface_format,
    create_platform_surface, destroy_failed_surface,
};

pub(crate) fn vk_err(operation: &str, err: vk::Result) -> Error {
    let code = match err {
        vk::Result::ERROR_OUT_OF_DATE_KHR
        | vk::Result::SUBOPTIMAL_KHR
        | vk::Result::ERROR_SURFACE_LOST_KHR => Errc::GraphicsSurfaceLost,
        vk::Result::ERROR_DEVICE_LOST => Errc::GraphicsDeviceLost,
        vk::Result::ERROR_OUT_OF_DEVICE_MEMORY | vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
            Errc::GraphicsOutOfMemory
        }
        _ => Errc::PlatformError,
    };
    Error::new(code, format!("VulkanContext: {operation} failed: {err:?}"))
}

/// 校验 teardown 前的 device idle 结果。
///
/// Vulkan 把 `ERROR_DEVICE_LOST` 视为 pending 资源不再 in-use，但 child object
/// 仍须显式销毁；因此该状态允许继续按子到父的顺序回收。其余 wait 失败保留 typed 根因。
pub(crate) fn accept_device_wait_for_shutdown(
    result: std::result::Result<(), vk::Result>,
) -> Result<()> {
    match result {
        Ok(()) | Err(vk::Result::ERROR_DEVICE_LOST) => Ok(()),
        Err(err) => Err(vk_err("vkDeviceWaitIdle during shutdown", err)),
    }
}

/// 架构守卫的拆分后源边界：`surface.rs` 持有 `create_win32_surface`、
/// `create_wayland_surface`、`create_metal_surface` 与 `portability_enumeration`；
/// `adapter.rs` 持有 `portability_subset`。返回源码仅供测试核对真实所有权。
#[cfg(test)]
pub(crate) const fn platform_contract_sources() -> (&'static str, &'static str) {
    (include_str!("surface.rs"), include_str!("adapter.rs"))
}

#[cfg(test)]
pub(crate) const fn drawable_contract_source() -> &'static str {
    include_str!("drawable.rs")
}

pub(super) fn loader_err(operation: &str, err: impl std::fmt::Debug) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("VulkanContext: {operation} failed: {err:?}"),
    )
}

pub(super) fn invalid(message: impl Into<String>) -> Error {
    Error::new(Errc::InvalidArgument, message.into())
}

struct UploadBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: vk::DeviceSize,
}

pub struct VulkanContext {
    runtime: Option<Rc<VulkanRuntime>>,
    device_lease: Option<Rc<VulkanDevice>>,
    surface_loader: ash::khr::surface::Instance,
    #[cfg(all(unix, not(target_os = "macos")))]
    _wayland_surface_loader: ash::khr::wayland_surface::Instance,
    #[cfg(windows)]
    _win32_surface_loader: ash::khr::win32_surface::Instance,
    #[cfg(target_os = "macos")]
    _metal_surface_loader: ash::ext::metal_surface::Instance,
    /// AppKit-owned `CAMetalLayer` (macOS MoltenVK WSI); used to sync drawableSize.
    #[cfg(target_os = "macos")]
    metal_layer: *mut c_void,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
    pub(crate) adapter_info: VulkanAdapterInfo,
    device: ash::Device,
    queue: vk::Queue,
    swapchain_loader: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    swapchain_images: Vec<vk::Image>,
    image_layouts: Vec<vk::ImageLayout>,
    swapchain_format: vk::Format,
    extent: vk::Extent2D,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    upload: UploadBuffer,
    image_available: vk::Semaphore,
    render_finished: vk::Semaphore,
    frame_fence: vk::Fence,
    native_surface: *mut c_void,
    logical_width: i32,
    logical_height: i32,
    width: i32,
    height: i32,
    /// Last successfully staged PixelUpload frame (CPU shadow of staging buffer).
    /// Enables destination-dependent IR via readback → apply → replace upload without
    /// claiming GPU image readback.
    cpu_shadow: Vec<u32>,
    shutdown: bool,
}

impl VulkanContext {
    pub(crate) fn new(native_surface: *mut c_void, width: i32, height: i32) -> Result<Self> {
        let drawable = drawable_size(native_surface, width, height);
        let extent = vk::Extent2D {
            width: drawable.width as u32,
            height: drawable.height as u32,
        };

        let runtime = VulkanRuntime::acquire()?;
        let instance = runtime.instance().clone();
        let surface_loader = ash::khr::surface::Instance::new(runtime.entry(), &instance);
        let (surface, platform_loader) =
            create_platform_surface(runtime.entry(), &instance, native_surface)?;

        let selection = match select_queue(&instance, runtime.surface_loader(), surface) {
            Ok(selection) => selection,
            Err(err) => {
                destroy_failed_surface(&surface_loader, surface);
                return Err(err);
            }
        };
        let queue_family_index = selection.family_index;
        let device_lease = match runtime.acquire_device(selection) {
            Ok(device) => device,
            Err(err) => {
                destroy_failed_surface(&surface_loader, surface);
                return Err(err);
            }
        };
        let physical_device = device_lease.physical_device();
        let adapter_info = device_lease.info().clone();
        let device = device_lease.device().clone();
        let queue = device_lease.queue();
        let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);
        let command_pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = match unsafe { device.create_command_pool(&command_pool_info, None) } {
            Ok(pool) => pool,
            Err(err) => {
                destroy_failed_surface(&surface_loader, surface);
                return Err(device_lease.error("vkCreateCommandPool", err));
            }
        };
        let command_alloc = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let command_buffer = match unsafe { device.allocate_command_buffers(&command_alloc) } {
            Ok(mut buffers) => buffers.pop(),
            Err(err) => {
                unsafe {
                    device.destroy_command_pool(command_pool, None);
                }
                destroy_failed_surface(&surface_loader, surface);
                return Err(device_lease.error("vkAllocateCommandBuffers", err));
            }
        }
        .ok_or_else(|| {
            unsafe {
                device.destroy_command_pool(command_pool, None);
            }
            destroy_failed_surface(&surface_loader, surface);
            Error::new(Errc::PlatformError, "VulkanContext: no command buffer")
        })?;

        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let image_available = match unsafe { device.create_semaphore(&semaphore_info, None) } {
            Ok(sem) => sem,
            Err(err) => {
                unsafe {
                    device.destroy_command_pool(command_pool, None);
                }
                destroy_failed_surface(&surface_loader, surface);
                return Err(device_lease.error("vkCreateSemaphore image_available", err));
            }
        };
        let render_finished = match unsafe { device.create_semaphore(&semaphore_info, None) } {
            Ok(sem) => sem,
            Err(err) => {
                unsafe {
                    device.destroy_semaphore(image_available, None);
                    device.destroy_command_pool(command_pool, None);
                }
                destroy_failed_surface(&surface_loader, surface);
                return Err(device_lease.error("vkCreateSemaphore render_finished", err));
            }
        };
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        let frame_fence = match unsafe { device.create_fence(&fence_info, None) } {
            Ok(fence) => fence,
            Err(err) => {
                unsafe {
                    device.destroy_semaphore(render_finished, None);
                    device.destroy_semaphore(image_available, None);
                    device.destroy_command_pool(command_pool, None);
                }
                destroy_failed_surface(&surface_loader, surface);
                return Err(device_lease.error("vkCreateFence", err));
            }
        };

        let mut ctx = Self {
            runtime: Some(runtime),
            device_lease: Some(device_lease),
            surface_loader,
            #[cfg(all(unix, not(target_os = "macos")))]
            _wayland_surface_loader: platform_loader,
            #[cfg(windows)]
            _win32_surface_loader: platform_loader,
            #[cfg(target_os = "macos")]
            _metal_surface_loader: platform_loader,
            #[cfg(target_os = "macos")]
            metal_layer: native_surface,
            surface,
            physical_device,
            adapter_info,
            device,
            queue,
            swapchain_loader,
            swapchain: vk::SwapchainKHR::null(),
            swapchain_images: Vec::new(),
            image_layouts: Vec::new(),
            swapchain_format: vk::Format::UNDEFINED,
            extent,
            command_pool,
            command_buffer,
            upload: UploadBuffer {
                buffer: vk::Buffer::null(),
                memory: vk::DeviceMemory::null(),
                size: 0,
            },
            image_available,
            render_finished,
            frame_fence,
            native_surface,
            logical_width: drawable.logical_width,
            logical_height: drawable.logical_height,
            width: drawable.width,
            height: drawable.height,
            cpu_shadow: Vec::new(),
            shutdown: false,
        };
        let device = ctx.active_device()?;
        device.observe(ctx.recreate_swapchain(extent))?;
        ctx.width = ctx.extent.width as i32;
        ctx.height = ctx.extent.height as i32;
        device.observe(ctx.recreate_upload_buffer(staging_size(ctx.width, ctx.height)))?;
        crate::core::log::info_fn(format!(
            "VulkanContext: created {}x{} swapchain; {}",
            ctx.width,
            ctx.height,
            ctx.adapter_info.diagnostic_summary()
        ));
        Ok(ctx)
    }

    fn active_device(&self) -> Result<Rc<VulkanDevice>> {
        self.device_lease.as_ref().cloned().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "VulkanContext: operation requested after shutdown",
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn shared_device_identity(&self) -> usize {
        self.device_lease
            .as_ref()
            .map_or(0, |device| Rc::as_ptr(device) as usize)
    }

    #[cfg(test)]
    pub(crate) fn mark_shared_device_lost_for_test(&self) {
        if let Some(device) = self.device_lease.as_ref() {
            device.mark_lost();
        }
    }

    fn resize_active(&mut self, width: i32, height: i32) -> Result<()> {
        let drawable = drawable_size(self.native_surface, width, height);
        if drawable.width == self.width && drawable.height == self.height {
            self.logical_width = drawable.logical_width;
            self.logical_height = drawable.logical_height;
            return Ok(());
        }
        let extent = vk::Extent2D {
            width: drawable.width as u32,
            height: drawable.height as u32,
        };
        self.recreate_swapchain(extent)?;
        self.logical_width = drawable.logical_width;
        self.logical_height = drawable.logical_height;
        self.width = self.extent.width as i32;
        self.height = self.extent.height as i32;
        self.cpu_shadow.clear();
        Ok(())
    }

    fn recreate_swapchain(&mut self, extent: vk::Extent2D) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            // MoltenVK surface capabilities follow CAMetalLayer.drawableSize.
            // SAFETY: metal_layer is the AppKit-owned layer passed as native_surface.
            unsafe {
                macos_surface::set_metal_layer_drawable_size(
                    self.metal_layer,
                    extent.width as i32,
                    extent.height as i32,
                );
            }
        }
        let old_swapchain = self.swapchain;
        if old_swapchain != vk::SwapchainKHR::null() {
            unsafe {
                self.device
                    .device_wait_idle()
                    .map_err(|err| vk_err("vkDeviceWaitIdle before swapchain recreate", err))?;
            }
        }

        let caps = unsafe {
            self.surface_loader
                .get_physical_device_surface_capabilities(self.physical_device, self.surface)
        }
        .map_err(|err| vk_err("vkGetPhysicalDeviceSurfaceCapabilitiesKHR", err))?;
        let formats = unsafe {
            self.surface_loader
                .get_physical_device_surface_formats(self.physical_device, self.surface)
        }
        .map_err(|err| vk_err("vkGetPhysicalDeviceSurfaceFormatsKHR", err))?;
        let present_modes = unsafe {
            self.surface_loader
                .get_physical_device_surface_present_modes(self.physical_device, self.surface)
        }
        .map_err(|err| vk_err("vkGetPhysicalDeviceSurfacePresentModesKHR", err))?;

        let surface_format = choose_surface_format(&formats);
        let present_mode = choose_present_mode(&present_modes);
        let extent = choose_extent(caps, extent);
        if !caps
            .supported_usage_flags
            .contains(vk::ImageUsageFlags::TRANSFER_DST)
        {
            return Err(Error::new(
                Errc::PlatformError,
                "VulkanContext: surface swapchain images do not support TRANSFER_DST",
            ));
        }
        let composite_alpha =
            choose_composite_alpha(caps.supported_composite_alpha).ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "VulkanContext: surface has no supported composite alpha mode",
                )
            })?;
        let desired_images = caps.min_image_count.saturating_add(1).max(2);
        let image_count = if caps.max_image_count > 0 {
            desired_images.min(caps.max_image_count)
        } else {
            desired_images
        };
        let pre_transform = if caps
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            caps.current_transform
        };
        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(self.surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::TRANSFER_DST)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(pre_transform)
            .composite_alpha(composite_alpha)
            .present_mode(present_mode)
            .old_swapchain(old_swapchain)
            .clipped(true);
        let new_swapchain = unsafe { self.swapchain_loader.create_swapchain(&create_info, None) }
            .map_err(|err| vk_err("vkCreateSwapchainKHR", err))?;
        let new_images = match unsafe { self.swapchain_loader.get_swapchain_images(new_swapchain) }
        {
            Ok(images) => images,
            Err(err) => {
                unsafe {
                    self.swapchain_loader.destroy_swapchain(new_swapchain, None);
                }
                return Err(vk_err("vkGetSwapchainImagesKHR", err));
            }
        };
        if old_swapchain != vk::SwapchainKHR::null() {
            unsafe {
                self.swapchain_loader.destroy_swapchain(old_swapchain, None);
            }
        }
        self.swapchain = new_swapchain;
        self.swapchain_images = new_images;
        self.image_layouts = vec![vk::ImageLayout::UNDEFINED; self.swapchain_images.len()];
        self.swapchain_format = surface_format.format;
        self.extent = extent;
        Ok(())
    }

    fn recreate_upload_buffer(&mut self, size: vk::DeviceSize) -> Result<()> {
        if self.upload.size >= size && self.upload.buffer != vk::Buffer::null() {
            return Ok(());
        }
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let buffer = unsafe { self.device.create_buffer(&buffer_info, None) }
            .map_err(|err| vk_err("vkCreateBuffer", err))?;
        let requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };
        let runtime = self.runtime.as_ref().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "VulkanContext: upload buffer requested after shutdown",
            )
        })?;
        let memory_index = match find_memory_type(
            runtime.instance(),
            self.physical_device,
            requirements.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        ) {
            Ok(index) => index,
            Err(error) => {
                unsafe { self.device.destroy_buffer(buffer, None) };
                return Err(error);
            }
        };
        let alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_index);
        let memory = match unsafe { self.device.allocate_memory(&alloc, None) } {
            Ok(memory) => memory,
            Err(err) => {
                unsafe { self.device.destroy_buffer(buffer, None) };
                return Err(vk_err("vkAllocateMemory staging", err));
            }
        };
        if let Err(err) = unsafe { self.device.bind_buffer_memory(buffer, memory, 0) } {
            unsafe {
                self.device.free_memory(memory, None);
                self.device.destroy_buffer(buffer, None);
            }
            return Err(vk_err("vkBindBufferMemory", err));
        }
        let previous = std::mem::replace(
            &mut self.upload,
            UploadBuffer {
                buffer,
                memory,
                size: requirements.size,
            },
        );
        unsafe {
            if previous.buffer != vk::Buffer::null() {
                self.device.destroy_buffer(previous.buffer, None);
            }
            if previous.memory != vk::DeviceMemory::null() {
                self.device.free_memory(previous.memory, None);
            }
        }
        Ok(())
    }

    fn upload_pixels(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<()> {
        if width != self.extent.width as i32 || height != self.extent.height as i32 {
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                format!(
                    "VulkanContext: pixel extent {width}x{height} no longer matches swapchain {}x{}",
                    self.extent.width, self.extent.height
                ),
            ));
        }
        let needed_pixels = (width as usize).saturating_mul(height as usize);
        if pixels.len() < needed_pixels {
            return Err(invalid(format!(
                "VulkanContext: pixel buffer too small, got {}, need {needed_pixels}",
                pixels.len()
            )));
        }
        let needed_size = staging_size(width, height);
        self.recreate_upload_buffer(needed_size)?;
        let copy_t0 = std::time::Instant::now();
        unsafe {
            let mapped = self
                .device
                .map_memory(
                    self.upload.memory,
                    0,
                    needed_size,
                    vk::MemoryMapFlags::empty(),
                )
                .map_err(|err| vk_err("vkMapMemory staging", err))?;
            ptr::copy_nonoverlapping(
                pixels.as_ptr().cast::<u8>(),
                mapped.cast::<u8>(),
                needed_size as usize,
            );
            self.device.unmap_memory(self.upload.memory);
        }
        let upload_copy_us = copy_t0.elapsed().as_micros();
        let mut sample = crate::core::perf_probe::take_present();
        sample.upload_copy_us = upload_copy_us;
        crate::core::perf_probe::record_present(sample);
        // 热路径不每帧全量复制 CPU shadow（约等于再拷一遍全屏）；
        // destination-dependent readback 时再 hydrate。
        self.cpu_shadow.clear();
        Ok(())
    }

    fn hydrate_cpu_shadow_from_staging(&mut self) -> Result<()> {
        let needed_pixels = (self.width as usize).saturating_mul(self.height as usize);
        if needed_pixels == 0 {
            return Ok(());
        }
        if self.cpu_shadow.len() == needed_pixels {
            return Ok(());
        }
        let needed_size = staging_size(self.width, self.height);
        if self.upload.buffer == vk::Buffer::null() || self.upload.size < needed_size {
            return Err(Error::new(
                Errc::InvalidState,
                "VulkanContext: no uploaded frame to read back (staging empty)",
            ));
        }
        unsafe {
            let mapped = self
                .device
                .map_memory(
                    self.upload.memory,
                    0,
                    needed_size,
                    vk::MemoryMapFlags::empty(),
                )
                .map_err(|err| vk_err("vkMapMemory staging hydrate", err))?;
            self.cpu_shadow.clear();
            self.cpu_shadow.resize(needed_pixels, 0);
            ptr::copy_nonoverlapping(
                mapped.cast::<u8>(),
                self.cpu_shadow.as_mut_ptr().cast::<u8>(),
                needed_pixels.saturating_mul(4),
            );
            self.device.unmap_memory(self.upload.memory);
        }
        Ok(())
    }

    fn present_uploaded_pixels(&mut self) -> Result<()> {
        let fence_t0 = std::time::Instant::now();
        unsafe {
            self.device
                .wait_for_fences(&[self.frame_fence], true, u64::MAX)
                .map_err(|err| vk_err("vkWaitForFences", err))?;
        }
        let fence_wait_us = fence_t0.elapsed().as_micros();
        let submit_t0 = std::time::Instant::now();
        let (image_index, acquire_suboptimal) = match unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available,
                vk::Fence::null(),
            )
        } {
            Ok((index, suboptimal)) => (index, suboptimal),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                return self.recreate_after_surface_change(
                    "vkAcquireNextImageKHR",
                    vk::Result::ERROR_OUT_OF_DATE_KHR,
                );
            }
            Err(err) => return Err(vk_err("vkAcquireNextImageKHR", err)),
        };

        self.record_upload_commands(image_index as usize)?;
        unsafe {
            self.device
                .reset_fences(&[self.frame_fence])
                .map_err(|err| vk_err("vkResetFences", err))?;
        }
        let wait_stages = [vk::PipelineStageFlags::TRANSFER];
        let submit = vk::SubmitInfo::default()
            .wait_semaphores(std::slice::from_ref(&self.image_available))
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(std::slice::from_ref(&self.command_buffer))
            .signal_semaphores(std::slice::from_ref(&self.render_finished));
        if let Err(err) = unsafe {
            self.device
                .queue_submit(self.queue, std::slice::from_ref(&submit), self.frame_fence)
        } {
            self.restore_signaled_frame_fence()?;
            return Err(vk_err("vkQueueSubmit", err));
        }
        self.image_layouts[image_index as usize] = vk::ImageLayout::PRESENT_SRC_KHR;
        let present = vk::PresentInfoKHR::default()
            .wait_semaphores(std::slice::from_ref(&self.render_finished))
            .swapchains(std::slice::from_ref(&self.swapchain))
            .image_indices(std::slice::from_ref(&image_index));
        let present_match = unsafe { self.swapchain_loader.queue_present(self.queue, &present) };
        let submit_present_us = submit_t0.elapsed().as_micros();
        let mut sample = crate::core::perf_probe::take_present();
        sample.fence_wait_us = fence_wait_us;
        sample.submit_present_us = submit_present_us;
        crate::core::perf_probe::record_present(sample);
        match present_match {
            Ok(present_suboptimal) if acquire_suboptimal || present_suboptimal => {
                self.recreate_after_surface_change("vkQueuePresentKHR", vk::Result::SUBOPTIMAL_KHR)
            }
            Ok(_) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.recreate_after_surface_change(
                "vkQueuePresentKHR",
                vk::Result::ERROR_OUT_OF_DATE_KHR,
            ),
            Err(vk::Result::SUBOPTIMAL_KHR) => {
                self.recreate_after_surface_change("vkQueuePresentKHR", vk::Result::SUBOPTIMAL_KHR)
            }
            Err(err) => Err(vk_err("vkQueuePresentKHR", err)),
        }
    }

    /// A swapchain status requires recreation, but the frame that observed it
    /// was not presented. Return a typed failure after a successful rebuild so
    /// the engine preserves dirty state and retries on the next frame.
    fn recreate_after_surface_change(&mut self, operation: &str, status: vk::Result) -> Result<()> {
        let surface_failure = vk_err(operation, status);
        match self.recreate_swapchain(self.extent) {
            Ok(()) => Err(surface_failure),
            Err(recreate_failure) => Err(surface_failure.with_source(recreate_failure)),
        }
    }

    fn record_upload_commands(&mut self, image_index: usize) -> Result<()> {
        let image = self.swapchain_images[image_index];
        let old_layout = self.image_layouts[image_index];
        let range = color_subresource_range();
        let begin = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe {
            self.device
                .reset_command_buffer(self.command_buffer, vk::CommandBufferResetFlags::empty())
                .map_err(|err| vk_err("vkResetCommandBuffer", err))?;
            self.device
                .begin_command_buffer(self.command_buffer, &begin)
                .map_err(|err| vk_err("vkBeginCommandBuffer", err))?;
            let to_transfer = vk::ImageMemoryBarrier::default()
                .old_layout(old_layout)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image)
                .subresource_range(range)
                .src_access_mask(if old_layout == vk::ImageLayout::UNDEFINED {
                    vk::AccessFlags::empty()
                } else {
                    vk::AccessFlags::MEMORY_READ
                })
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);
            self.device.cmd_pipeline_barrier(
                self.command_buffer,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                std::slice::from_ref(&to_transfer),
            );
            let copy = vk::BufferImageCopy::default()
                .buffer_offset(0)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(
                    vk::ImageSubresourceLayers::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .mip_level(0)
                        .base_array_layer(0)
                        .layer_count(1),
                )
                .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                .image_extent(vk::Extent3D {
                    width: self.extent.width,
                    height: self.extent.height,
                    depth: 1,
                });
            self.device.cmd_copy_buffer_to_image(
                self.command_buffer,
                self.upload.buffer,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                std::slice::from_ref(&copy),
            );
            let to_present = vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(image)
                .subresource_range(range)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::MEMORY_READ);
            self.device.cmd_pipeline_barrier(
                self.command_buffer,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                std::slice::from_ref(&to_present),
            );
            self.device
                .end_command_buffer(self.command_buffer)
                .map_err(|err| vk_err("vkEndCommandBuffer", err))?;
        }
        Ok(())
    }

    /// `vkResetFences` makes the fence unsignaled before submit. If submit
    /// itself fails, no queue operation will signal it, so replace it with a
    /// fresh signaled fence before returning a typed failure.
    fn restore_signaled_frame_fence(&mut self) -> Result<()> {
        let replacement = unsafe {
            self.device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )
        }
        .map_err(|err| vk_err("vkCreateFence after failed submit", err))?;
        let previous = std::mem::replace(&mut self.frame_fence, replacement);
        unsafe {
            self.device.destroy_fence(previous, None);
        }
        Ok(())
    }

    fn shutdown_result(&mut self) -> Result<()> {
        if self.shutdown {
            return Ok(());
        }
        // 当前 context 的提交在返回前已由 frame fence 排空；device lost 时
        // Vulkan 仍要求显式销毁本窗口拥有的 child object。
        let device = self.active_device()?;
        let wait_result = unsafe { self.device.device_wait_idle() };
        device.observe_wait(wait_result);
        accept_device_wait_for_shutdown(wait_result)?;
        unsafe {
            if self.upload.buffer != vk::Buffer::null() {
                self.device.destroy_buffer(self.upload.buffer, None);
            }
            if self.upload.memory != vk::DeviceMemory::null() {
                self.device.free_memory(self.upload.memory, None);
            }
            if self.frame_fence != vk::Fence::null() {
                self.device.destroy_fence(self.frame_fence, None);
            }
            if self.render_finished != vk::Semaphore::null() {
                self.device.destroy_semaphore(self.render_finished, None);
            }
            if self.image_available != vk::Semaphore::null() {
                self.device.destroy_semaphore(self.image_available, None);
            }
            if self.command_pool != vk::CommandPool::null() {
                self.device.destroy_command_pool(self.command_pool, None);
            }
            if self.swapchain != vk::SwapchainKHR::null() {
                self.swapchain_loader
                    .destroy_swapchain(self.swapchain, None);
            }
            if self.surface != vk::SurfaceKHR::null() {
                self.surface_loader.destroy_surface(self.surface, None);
            }
        }
        self.device_lease.take();
        self.runtime.take();
        self.shutdown = true;
        Ok(())
    }
}

impl IGraphicsContext for VulkanContext {
    fn caps(&self) -> crate::native::traits::present::GraphicsContextCaps {
        GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::Vulkan, self.device_pixel_ratio())
    }

    fn graphics_backend(&self) -> GraphicsBackend {
        GraphicsBackend::Vulkan
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        let device = self.active_device()?;
        device.ensure_healthy()?;
        let result = self.resize_active(width, height);
        device.observe(result)
    }

    fn make_current(&mut self) -> Result<()> {
        Ok(())
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "VulkanContext: swapchain present is not supported for the PixelUpload recipe; use PresentFrame::PixelBuffer",
        ))
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>> {
        let device = self.active_device()?;
        device.ensure_healthy()?;
        let result = (|| {
            self.hydrate_cpu_shadow_from_staging()?;
            let expected = (self.width as usize).saturating_mul(self.height as usize);
            if self.cpu_shadow.len() != expected {
                return Err(Error::new(
                    Errc::InvalidState,
                    "VulkanContext: no uploaded frame to read back (cpu_shadow empty)",
                ));
            }
            crop_cpu_shadow(
                &self.cpu_shadow,
                self.width,
                self.height,
                x,
                y,
                width,
                height,
            )
        })();
        device.observe(result)
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.width as f32 / self.logical_width.max(1) as f32
    }

    fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        _damage: PresentDamage,
    ) -> Result<()> {
        let device = self.active_device()?;
        device.ensure_healthy()?;
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let result = self
            .upload_pixels(pixels, width, height)
            .and_then(|()| self.present_uploaded_pixels());
        device.observe(result)
    }

    /// Stage a full replace pixel buffer into the upload heap without presenting.
    /// Also refreshes the CPU shadow used by [`Self::read_pixels`] so destination-
    /// dependent IR can round-trip through readback → apply → replace upload.
    fn upload_surface_pixels(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<()> {
        let device = self.active_device()?;
        device.ensure_healthy()?;
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let result = self.upload_pixels(pixels, width, height);
        device.observe(result)
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        if let Err(error) = self.try_shutdown() {
            crate::core::log::error_fn(format!(
                "VulkanContext: undrained Drop retained Vulkan parents: {}",
                error.short_what()
            ));
            if let Some(device) = self.device_lease.take() {
                std::mem::forget(device);
            }
            if let Some(runtime) = self.runtime.take() {
                std::mem::forget(runtime);
            }
        }
    }
}

pub(crate) fn crop_cpu_shadow(
    shadow: &[u32],
    surface_width: i32,
    surface_height: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Result<Vec<u32>> {
    if width <= 0 || height <= 0 {
        return Ok(Vec::new());
    }
    if x < 0
        || y < 0
        || x.saturating_add(width) > surface_width
        || y.saturating_add(height) > surface_height
    {
        return Err(invalid(format!(
            "VulkanContext: readback rect ({x},{y},{width}x{height}) outside {surface_width}x{surface_height}"
        )));
    }
    let mut out = Vec::with_capacity((width as usize).saturating_mul(height as usize));
    let stride = surface_width as usize;
    for row in 0..height as usize {
        let start = (y as usize + row).saturating_mul(stride) + x as usize;
        let end = start + width as usize;
        out.extend_from_slice(&shadow[start..end]);
    }
    Ok(out)
}

fn find_memory_type(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    type_bits: u32,
    flags: vk::MemoryPropertyFlags,
) -> Result<u32> {
    let props = unsafe { instance.get_physical_device_memory_properties(physical_device) };
    for index in 0..props.memory_type_count {
        let supported = (type_bits & (1 << index)) != 0;
        let has_flags = props.memory_types[index as usize]
            .property_flags
            .contains(flags);
        if supported && has_flags {
            return Ok(index);
        }
    }
    Err(Error::new(
        Errc::PlatformError,
        "VulkanContext: no host-visible coherent staging memory type",
    ))
}

fn color_subresource_range() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1)
}
