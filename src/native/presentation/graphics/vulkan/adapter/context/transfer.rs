//! Vulkan PixelUpload staging memory and transfer command ownership.

use std::ptr;

use ash::vk;

use crate::core::{Errc, Error, Result};

use super::super::rhi::color_subresource_range;
use super::{UploadBuffer, VulkanContext, invalid, staging_size, vk_err};

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
        // SAFETY: device 存活；buffer_info 为栈上完整初始化的创建描述；分配器传 None。
        let buffer = unsafe { self.device.create_buffer(&buffer_info, None) }
            .map_err(|err| vk_err("vkCreateBuffer", err))?;
        // SAFETY: buffer 为刚创建的存活对象，查询内存需求为只读操作。
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
            "host-visible coherent staging",
        ) {
            Ok(index) => index,
            Err(error) => {
                // SAFETY: buffer 为本函数刚创建、仍存活且失败后不再使用。
                unsafe { self.device.destroy_buffer(buffer, None) };
                return Err(error);
            }
        };
        let alloc = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_index);
        // SAFETY: device 存活；alloc 为栈上完整初始化的分配描述；分配器传 None。
        let memory = match unsafe { self.device.allocate_memory(&alloc, None) } {
            Ok(memory) => memory,
            Err(err) => {
                // SAFETY: buffer 为本函数刚创建、仍存活且失败后不再使用。
                unsafe { self.device.destroy_buffer(buffer, None) };
                return Err(vk_err("vkAllocateMemory staging", err));
            }
        };
        // SAFETY: buffer/memory 均为刚创建且匹配（由同一次分配绑定）；偏移 0 有效。
        if let Err(err) = unsafe { self.device.bind_buffer_memory(buffer, memory, 0) } {
            // SAFETY: 绑定失败时两者仍存活且不再使用，按创建逆序释放。
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
        // SAFETY: previous 中的对象被替换出来后仍存活且不再被引用，先删 buffer 再释放内存。
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
        // SAFETY: upload.memory 存活且未被映射；needed_size 不超过分配大小；pixels 切片长度已在上方验证足够；mapped 指针由 map/unmap 对保证有效。
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
        // 热路径不每帧全量复制 CPU shadow（约等于再拷一遍全屏）；
        // destination-dependent readback 时再 hydrate。
        self.cpu_shadow.clear();
        Ok(())
    }

    /// 单 staging buffer 会被连续帧复用；CPU 覆写或替换前必须确认上一提交已停止读取。
    fn wait_for_previous_upload(&mut self) -> Result<()> {
        self.wait_for_frame_fence("vkWaitForFences before staging upload")?;
        if !self.present_fences.enabled() {
            self.present_lifetime
                .complete_submission(&self.device, &self.swapchain_loader)?;
        }
        Ok(())
    }

    pub(super) fn wait_for_frame_fence(&self, operation: &str) -> Result<()> {
        // SAFETY: frame_fence belongs to device and covers the only outstanding
        // upload submission owned by this window context.
        unsafe {
            self.device
                .wait_for_fences(&[self.frame_fence], true, u64::MAX)
                .map_err(|error| vk_err(operation, error))
        }
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
        // SAFETY: upload.memory 存活且未被映射；shadow 容量与 needed_pixels 匹配；mapped 指针由 map/unmap 对保证有效。
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
        // SAFETY: command_buffer 存活且未被录制中；image 为当前 swapchain 存活 image；barrier/copy 描述引用栈上对象且在命令提交前有效。
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
        // SAFETY: device 存活；fence 创建描述为栈上完整初始化；分配器传 None。
        let replacement = unsafe {
            self.device.create_fence(
                &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                None,
            )
        }
        .map_err(|err| vk_err("vkCreateFence after failed submit", err))?;
        let previous = std::mem::replace(&mut self.frame_fence, replacement);
        // SAFETY: previous 为替换出来的旧 fence，仍存活且不再被引用，只销毁一次。
        unsafe {
            self.device.destroy_fence(previous, None);
        }
        Ok(())
    }
}

pub(super) fn find_memory_type(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    type_bits: u32,
    flags: vk::MemoryPropertyFlags,
    purpose: &'static str,
) -> Result<u32> {
    // SAFETY: instance 存活且 physical_device 有效，查询内存属性为只读操作。
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
        format!("VulkanContext: no {purpose} memory type"),
    ))
}
