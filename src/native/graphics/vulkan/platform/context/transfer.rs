//! Vulkan PixelUpload staging memory and transfer command ownership.

use std::ptr;

use ash::vk;

use crate::core::{Errc, Error, Result};

use super::{invalid, staging_size, vk_err, UploadBuffer, VulkanContext};

pub(crate) fn allocate_cpu_shadow(pixel_count: usize) -> Result<Vec<u32>> {
    let mut shadow = Vec::new();
    shadow.try_reserve_exact(pixel_count).map_err(|error| {
        Error::new(
            Errc::GraphicsOutOfMemory,
            format!(
                "VulkanContext: CPU readback shadow allocation for {pixel_count} pixels failed: {error}"
            ),
        )
    })?;
    shadow.resize(pixel_count, 0);
    Ok(shadow)
}

impl VulkanContext {
    pub(super) fn recreate_upload_buffer(&mut self, size: vk::DeviceSize) -> Result<()> {
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

    pub(super) fn upload_pixels(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<()> {
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
        self.wait_for_previous_upload()?;
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

    /// 单 staging buffer 会被连续帧复用；CPU 覆写或替换前必须确认上一提交已停止读取。
    fn wait_for_previous_upload(&self) -> Result<()> {
        let fence_t0 = std::time::Instant::now();
        unsafe {
            self.device
                .wait_for_fences(&[self.frame_fence], true, u64::MAX)
                .map_err(|err| vk_err("vkWaitForFences before staging upload", err))?;
        }
        let mut sample = crate::core::perf_probe::take_present();
        sample.fence_wait_us = fence_t0.elapsed().as_micros();
        crate::core::perf_probe::record_present(sample);
        Ok(())
    }

    pub(super) fn hydrate_cpu_shadow_from_staging(&mut self) -> Result<()> {
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
        let mut shadow = allocate_cpu_shadow(needed_pixels)?;
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
            ptr::copy_nonoverlapping(
                mapped.cast::<u8>(),
                shadow.as_mut_ptr().cast::<u8>(),
                needed_pixels.saturating_mul(4),
            );
            self.device.unmap_memory(self.upload.memory);
        }
        self.cpu_shadow = shadow;
        Ok(())
    }

    pub(super) fn record_upload_commands(&mut self, image_index: usize) -> Result<()> {
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
    pub(super) fn restore_signaled_frame_fence(&mut self) -> Result<()> {
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
