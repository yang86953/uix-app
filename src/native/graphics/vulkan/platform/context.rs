//! Vulkan graphics context for Linux Wayland.

use std::ffi::{CStr, c_void};
use std::ptr;

use ash::{Entry, vk};

use crate::core::{Errc, Error, Result};
use crate::native::traits::present::{
    GraphicsBackend, GraphicsContextCaps, IGraphicsContext, PresentDamage,
};

use crate::native::graphics::platform::linux::WaylandSurfaceHandle;

fn vk_err(operation: &str, err: vk::Result) -> Error {
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

fn loader_err(operation: &str, err: impl std::fmt::Debug) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("VulkanContext: {operation} failed: {err:?}"),
    )
}

fn invalid(message: impl Into<String>) -> Error {
    Error::new(Errc::InvalidArgument, message.into())
}

#[derive(Clone, Copy)]
struct QueueSelection {
    physical_device: vk::PhysicalDevice,
    family_index: u32,
}

struct UploadBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    size: vk::DeviceSize,
}

pub struct VulkanContext {
    _entry: Entry,
    instance: ash::Instance,
    surface_loader: ash::khr::surface::Instance,
    _wayland_surface_loader: ash::khr::wayland_surface::Instance,
    surface: vk::SurfaceKHR,
    physical_device: vk::PhysicalDevice,
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
    width: i32,
    height: i32,
    shutdown: bool,
}

impl VulkanContext {
    pub fn new(native_surface: *mut c_void, width: i32, height: i32) -> Result<Self> {
        let wayland = unsafe { WaylandSurfaceHandle::from_native(native_surface)? };
        let width = width.max(1);
        let height = height.max(1);
        let extent = vk::Extent2D {
            width: width as u32,
            height: height as u32,
        };

        let entry =
            unsafe { Entry::load() }.map_err(|err| loader_err("load Vulkan loader", err))?;
        let app_name = CStr::from_bytes_with_nul(b"uix\0").expect("static C string");
        let engine_name = CStr::from_bytes_with_nul(b"uix\0").expect("static C string");
        let app_info = vk::ApplicationInfo::default()
            .application_name(app_name)
            .application_version(1)
            .engine_name(engine_name)
            .engine_version(1)
            .api_version(vk::API_VERSION_1_0);
        let instance_extensions = [
            ash::khr::surface::NAME.as_ptr(),
            ash::khr::wayland_surface::NAME.as_ptr(),
        ];
        let instance_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&instance_extensions);
        let instance = unsafe { entry.create_instance(&instance_info, None) }
            .map_err(|err| vk_err("vkCreateInstance", err))?;

        let surface_loader = ash::khr::surface::Instance::new(&entry, &instance);
        let wayland_surface_loader = ash::khr::wayland_surface::Instance::new(&entry, &instance);
        let surface_info = vk::WaylandSurfaceCreateInfoKHR::default()
            .display(wayland.display.cast())
            .surface(wayland.surface.cast());
        let surface = unsafe { wayland_surface_loader.create_wayland_surface(&surface_info, None) }
            .map_err(|err| vk_err("vkCreateWaylandSurfaceKHR", err))?;

        let selection = select_queue(&instance, &surface_loader, surface)?;
        let queue_priority = [1.0_f32];
        let queue_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(selection.family_index)
            .queue_priorities(&queue_priority);
        let device_extensions = [ash::khr::swapchain::NAME.as_ptr()];
        let device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_info))
            .enabled_extension_names(&device_extensions);
        let device =
            unsafe { instance.create_device(selection.physical_device, &device_info, None) }
                .map_err(|err| vk_err("vkCreateDevice", err))?;
        let queue = unsafe { device.get_device_queue(selection.family_index, 0) };
        let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);
        let command_pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(selection.family_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = unsafe { device.create_command_pool(&command_pool_info, None) }
            .map_err(|err| vk_err("vkCreateCommandPool", err))?;
        let command_alloc = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let command_buffer = unsafe { device.allocate_command_buffers(&command_alloc) }
            .map_err(|err| vk_err("vkAllocateCommandBuffers", err))?
            .into_iter()
            .next()
            .ok_or_else(|| Error::new(Errc::PlatformError, "VulkanContext: no command buffer"))?;

        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let image_available = unsafe { device.create_semaphore(&semaphore_info, None) }
            .map_err(|err| vk_err("vkCreateSemaphore image_available", err))?;
        let render_finished = unsafe { device.create_semaphore(&semaphore_info, None) }
            .map_err(|err| vk_err("vkCreateSemaphore render_finished", err))?;
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        let frame_fence = unsafe { device.create_fence(&fence_info, None) }
            .map_err(|err| vk_err("vkCreateFence", err))?;

        let mut ctx = Self {
            _entry: entry,
            instance,
            surface_loader,
            _wayland_surface_loader: wayland_surface_loader,
            surface,
            physical_device: selection.physical_device,
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
            width,
            height,
            shutdown: false,
        };
        ctx.recreate_swapchain(extent)?;
        ctx.width = ctx.extent.width as i32;
        ctx.height = ctx.extent.height as i32;
        ctx.recreate_upload_buffer(staging_size(ctx.width, ctx.height))?;
        crate::core::log::info_fn(format!(
            "VulkanContext: created {}x{} swapchain",
            ctx.width, ctx.height
        ));
        Ok(ctx)
    }

    fn recreate_swapchain(&mut self, extent: vk::Extent2D) -> Result<()> {
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
        let memory_index = match find_memory_type(
            &self.instance,
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
        Ok(())
    }

    fn present_uploaded_pixels(&mut self) -> Result<()> {
        unsafe {
            self.device
                .wait_for_fences(&[self.frame_fence], true, u64::MAX)
                .map_err(|err| vk_err("vkWaitForFences", err))?;
        }
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
        match unsafe { self.swapchain_loader.queue_present(self.queue, &present) } {
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
        unsafe {
            self.device
                .device_wait_idle()
                .map_err(|err| vk_err("vkDeviceWaitIdle during shutdown", err))?;
        }
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
            self.device.destroy_device(None);
            if self.surface != vk::SurfaceKHR::null() {
                self.surface_loader.destroy_surface(self.surface, None);
            }
            self.instance.destroy_instance(None);
        }
        self.shutdown = true;
        Ok(())
    }
}

impl IGraphicsContext for VulkanContext {
    fn caps(&self) -> crate::native::traits::present::GraphicsContextCaps {
        GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::Vulkan, 1.0)
    }

    fn graphics_backend(&self) -> GraphicsBackend {
        GraphicsBackend::Vulkan
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        let width = width.max(1);
        let height = height.max(1);
        if width == self.width && height == self.height {
            return Ok(());
        }
        let extent = vk::Extent2D {
            width: width as u32,
            height: height as u32,
        };
        self.recreate_swapchain(extent)?;
        self.width = self.extent.width as i32;
        self.height = self.extent.height as i32;
        Ok(())
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

    fn shutdown(&mut self) {
        if let Err(error) = self.shutdown_result() {
            crate::core::log::error_fn(format!(
                "VulkanContext: shutdown failed: {}",
                error.short_what()
            ));
        }
    }

    fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Result<Vec<u32>> {
        Err(Error::new(
            Errc::NotImplemented,
            "VulkanContext: native readback is not supported",
        ))
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        _damage: PresentDamage,
    ) -> Result<()> {
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        if width != self.width || height != self.height {
            self.resize(width, height)?;
        }
        self.upload_pixels(pixels, width, height)?;
        self.present_uploaded_pixels()
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn select_queue(
    instance: &ash::Instance,
    surface_loader: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> Result<QueueSelection> {
    let physical_devices = unsafe { instance.enumerate_physical_devices() }
        .map_err(|err| vk_err("vkEnumeratePhysicalDevices", err))?;
    for physical_device in physical_devices {
        if !device_supports_swapchain(instance, physical_device)? {
            continue;
        }
        let queues =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        for (index, queue) in queues.iter().enumerate() {
            let supports_present = unsafe {
                surface_loader.get_physical_device_surface_support(
                    physical_device,
                    index as u32,
                    surface,
                )
            }
            .map_err(|err| vk_err("vkGetPhysicalDeviceSurfaceSupportKHR", err))?;
            if queue.queue_flags.contains(vk::QueueFlags::GRAPHICS) && supports_present {
                return Ok(QueueSelection {
                    physical_device,
                    family_index: index as u32,
                });
            }
        }
    }
    Err(Error::new(
        Errc::PlatformError,
        "VulkanContext: no graphics+present queue with VK_KHR_swapchain",
    ))
}

fn device_supports_swapchain(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> Result<bool> {
    let extensions = unsafe { instance.enumerate_device_extension_properties(physical_device) }
        .map_err(|err| vk_err("vkEnumerateDeviceExtensionProperties", err))?;
    Ok(extensions.iter().any(|extension| {
        let name = unsafe { CStr::from_ptr(extension.extension_name.as_ptr()) };
        name == ash::khr::swapchain::NAME
    }))
}

fn choose_surface_format(formats: &[vk::SurfaceFormatKHR]) -> vk::SurfaceFormatKHR {
    if formats.len() == 1 && formats[0].format == vk::Format::UNDEFINED {
        return vk::SurfaceFormatKHR {
            format: vk::Format::B8G8R8A8_UNORM,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        };
    }
    formats
        .iter()
        .copied()
        .find(|format| {
            format.format == vk::Format::B8G8R8A8_UNORM
                && format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
        })
        .or_else(|| formats.first().copied())
        .unwrap_or(vk::SurfaceFormatKHR {
            format: vk::Format::B8G8R8A8_UNORM,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        })
}

fn choose_present_mode(modes: &[vk::PresentModeKHR]) -> vk::PresentModeKHR {
    modes
        .iter()
        .copied()
        .find(|mode| *mode == vk::PresentModeKHR::FIFO)
        .or_else(|| modes.first().copied())
        .unwrap_or(vk::PresentModeKHR::FIFO)
}

fn choose_composite_alpha(
    supported: vk::CompositeAlphaFlagsKHR,
) -> Option<vk::CompositeAlphaFlagsKHR> {
    [
        vk::CompositeAlphaFlagsKHR::OPAQUE,
        vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
        vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
        vk::CompositeAlphaFlagsKHR::INHERIT,
    ]
    .into_iter()
    .find(|candidate| supported.contains(*candidate))
}

fn choose_extent(caps: vk::SurfaceCapabilitiesKHR, requested: vk::Extent2D) -> vk::Extent2D {
    if caps.current_extent.width != u32::MAX {
        return caps.current_extent;
    }
    vk::Extent2D {
        width: requested
            .width
            .clamp(caps.min_image_extent.width, caps.max_image_extent.width),
        height: requested
            .height
            .clamp(caps.min_image_extent.height, caps.max_image_extent.height),
    }
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

fn staging_size(width: i32, height: i32) -> vk::DeviceSize {
    (width.max(1) as vk::DeviceSize)
        .saturating_mul(height.max(1) as vk::DeviceSize)
        .saturating_mul(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choose_surface_format_prefers_bgra_srgb() {
        let formats = [
            vk::SurfaceFormatKHR {
                format: vk::Format::R8G8B8A8_UNORM,
                color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
            },
            vk::SurfaceFormatKHR {
                format: vk::Format::B8G8R8A8_UNORM,
                color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
            },
        ];
        assert!(choose_surface_format(&formats).format == vk::Format::B8G8R8A8_UNORM);
    }

    #[test]
    fn staging_size_is_full_rgba_frame() {
        assert_eq!(staging_size(4, 3), 48);
        assert_eq!(staging_size(0, 0), 4);
    }

    #[test]
    fn vulkan_present_statuses_are_typed_graphics_failures() {
        assert_eq!(
            vk_err("vkQueuePresentKHR", vk::Result::ERROR_OUT_OF_DATE_KHR).code(),
            Errc::GraphicsSurfaceLost
        );
        assert_eq!(
            vk_err("vkQueuePresentKHR", vk::Result::SUBOPTIMAL_KHR).code(),
            Errc::GraphicsSurfaceLost
        );
        assert_eq!(
            vk_err("vkQueuePresentKHR", vk::Result::ERROR_DEVICE_LOST).code(),
            Errc::GraphicsDeviceLost
        );
        assert_eq!(
            vk_err("vkQueuePresentKHR", vk::Result::ERROR_OUT_OF_DEVICE_MEMORY).code(),
            Errc::GraphicsOutOfMemory
        );
    }

    #[test]
    fn composite_alpha_prefers_opaque_but_uses_a_supported_fallback() {
        assert!(
            choose_composite_alpha(
                vk::CompositeAlphaFlagsKHR::OPAQUE | vk::CompositeAlphaFlagsKHR::INHERIT
            )
            .is_some_and(|mode| mode == vk::CompositeAlphaFlagsKHR::OPAQUE)
        );
        assert!(
            choose_composite_alpha(vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED)
                .is_some_and(|mode| mode == vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED)
        );
        assert!(choose_composite_alpha(vk::CompositeAlphaFlagsKHR::empty()).is_none());
    }
}
