//! Vulkan WSI Surface 的同步 staging 回读与像素规范化。
//!
//! 本组件只拥有原生 transfer destination；提交与等待复用同一 Context 已有的
//! `VulkanImmediateCommands`，区域与结果形状仍由 platform `RhiSurfaceReadback` 约束。

use ash::vk;

use crate::core::{Errc, Error, Result};
use crate::platform::presentation::rhi::RhiScissor;

use super::super::rhi::{color_subresource_layers, color_subresource_range};
use super::vk_err;

// 保存 Surface 回读专用的 HOST_VISIBLE transfer destination。
pub(super) struct VulkanSurfaceReadbackBuffer {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    buffer_capacity: vk::DeviceSize,
    allocation_size: vk::DeviceSize,
    coherent: bool,
}

impl VulkanSurfaceReadbackBuffer {
    pub(super) const fn new() -> Self {
        Self {
            buffer: vk::Buffer::null(),
            memory: vk::DeviceMemory::null(),
            buffer_capacity: 0,
            allocation_size: 0,
            coherent: false,
        }
    }

    // 按当前区域紧密像素载荷扩容；已完成的较大 staging 可跨 resize 复用。
    pub(super) fn ensure_capacity(
        &mut self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        required: vk::DeviceSize,
    ) -> Result<()> {
        if required == 0 {
            return Err(invalid_readback(
                "Vulkan Surface readback staging size must be positive",
            ));
        }
        if self.buffer != vk::Buffer::null() && self.buffer_capacity >= required {
            return Ok(());
        }

        let create_info = vk::BufferCreateInfo::default()
            .size(required)
            .usage(vk::BufferUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        // SAFETY: required 已证明非零；返回 Buffer 尚未进入正式 owner。
        let buffer = unsafe { device.create_buffer(&create_info, None) }
            .map_err(|error| vk_err("vkCreateBuffer Surface readback", error))?;
        // SAFETY: buffer 由当前 device 创建且仍存活。
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        let (memory_type_index, coherent) = match select_readback_memory_type(
            instance,
            physical_device,
            requirements.memory_type_bits,
        ) {
            Ok(selection) => selection,
            Err(error) => {
                // SAFETY: 尚未绑定内存的临时 Buffer 由本函数唯一拥有。
                unsafe { device.destroy_buffer(buffer, None) };
                return Err(error);
            }
        };
        let allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type_index);
        // SAFETY: 内存类型来自同一 physical device 的 Buffer requirements。
        let memory = match unsafe { device.allocate_memory(&allocate_info, None) } {
            Ok(memory) => memory,
            Err(error) => {
                // SAFETY: 分配失败后临时 Buffer 不再使用。
                unsafe { device.destroy_buffer(buffer, None) };
                return Err(vk_err("vkAllocateMemory Surface readback", error));
            }
        };
        // SAFETY: memory 容量与类型满足刚查询的 Buffer requirements。
        if let Err(error) = unsafe { device.bind_buffer_memory(buffer, memory, 0) } {
            // SAFETY: 绑定失败对象尚未发布，按创建逆序释放。
            unsafe {
                device.free_memory(memory, None);
                device.destroy_buffer(buffer, None);
            }
            return Err(vk_err("vkBindBufferMemory Surface readback", error));
        }

        let previous = std::mem::replace(
            self,
            Self {
                buffer,
                memory,
                buffer_capacity: required,
                allocation_size: requirements.size,
                coherent,
            },
        );
        // 调用方只在已完成的 immediate 事务间扩容，旧 staging 没有在途引用。
        previous.destroy(device);
        Ok(())
    }

    pub(super) const fn buffer(&self) -> vk::Buffer {
        self.buffer
    }

    // GPU 完成后映射紧密载荷，并按真实 swapchain format 规范化为 0xAARRGGBB。
    pub(super) fn read_pixels(
        &self,
        device: &ash::Device,
        format: vk::Format,
        pixel_count: usize,
    ) -> Result<Vec<u32>> {
        let byte_len = pixel_count
            .checked_mul(4)
            .ok_or_else(readback_size_overflow)?;
        let byte_len_u64 = u64::try_from(byte_len).map_err(|_| readback_size_overflow())?;
        if self.buffer == vk::Buffer::null()
            || byte_len_u64 > self.buffer_capacity
            || self.allocation_size == 0
        {
            return Err(invalid_readback(
                "Vulkan Surface readback staging is not large enough",
            ));
        }
        // SAFETY: memory 是 HOST_VISIBLE，映射整个 allocation 允许非 coherent 路径使用 WHOLE_SIZE。
        let mapped = unsafe {
            device.map_memory(
                self.memory,
                0,
                self.allocation_size,
                vk::MemoryMapFlags::empty(),
            )
        }
        .map_err(|error| vk_err("vkMapMemory Surface readback", error))?;
        if !self.coherent {
            let range = vk::MappedMemoryRange::default()
                .memory(self.memory)
                .offset(0)
                .size(vk::WHOLE_SIZE);
            // SAFETY: memory 当前已映射；WHOLE_SIZE 与零偏移满足 nonCoherentAtomSize 对齐。
            if let Err(error) =
                unsafe { device.invalidate_mapped_memory_ranges(std::slice::from_ref(&range)) }
            {
                // SAFETY: 失败路径仍须结束本函数建立的唯一映射。
                unsafe { device.unmap_memory(self.memory) };
                return Err(vk_err(
                    "vkInvalidateMappedMemoryRanges Surface readback",
                    error,
                ));
            }
        }
        // SAFETY: staging Buffer 大小至少为 byte_len，映射覆盖完整 allocation。
        let bytes = unsafe { std::slice::from_raw_parts(mapped.cast::<u8>(), byte_len) };
        let pixels = normalize_surface_pixels(format, bytes, pixel_count);
        // SAFETY: 本函数唯一拥有当前映射，像素已复制到 Rust Vec。
        unsafe { device.unmap_memory(self.memory) };
        pixels
    }

    pub(super) fn shutdown(&mut self, device: &ash::Device) {
        let previous = std::mem::replace(self, Self::new());
        previous.destroy(device);
    }

    fn destroy(self, device: &ash::Device) {
        // SAFETY: 调用方保证覆盖该 staging 的 queue 已完成，句柄只由本值拥有。
        unsafe {
            if self.buffer != vk::Buffer::null() {
                device.destroy_buffer(self.buffer, None);
            }
            if self.memory != vk::DeviceMemory::null() {
                device.free_memory(self.memory, None);
            }
        }
    }
}

// 返回当前 Adapter 可以无损规范化为 0xAARRGGBB 的真实 WSI 格式集合。
pub(super) const fn supports_surface_readback_format(format: vk::Format) -> bool {
    matches!(
        format,
        vk::Format::B8G8R8A8_UNORM
            | vk::Format::B8G8R8A8_SRGB
            | vk::Format::R8G8B8A8_UNORM
            | vk::Format::R8G8B8A8_SRGB
    )
}

// 为日志提供不依赖 ash Debug feature 的稳定 WSI 格式名称。
pub(super) const fn surface_readback_format_name(format: vk::Format) -> &'static str {
    match format {
        vk::Format::B8G8R8A8_UNORM => "B8G8R8A8_UNORM",
        vk::Format::B8G8R8A8_SRGB => "B8G8R8A8_SRGB",
        vk::Format::R8G8B8A8_UNORM => "R8G8B8A8_UNORM",
        vk::Format::R8G8B8A8_SRGB => "R8G8B8A8_SRGB",
        _ => "unsupported",
    }
}

// 记录 PRESENT → TRANSFER_SRC → PRESENT 与紧密区域 copy；提交和等待由 immediate owner 负责。
pub(super) fn record_surface_readback(
    device: &ash::Device,
    command: vk::CommandBuffer,
    image: vk::Image,
    destination: vk::Buffer,
    region: RhiScissor,
) {
    let subresource_range = color_subresource_range();
    let to_transfer = vk::ImageMemoryBarrier::default()
        .src_access_mask(vk::AccessFlags::MEMORY_READ)
        .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
        .old_layout(vk::ImageLayout::PRESENT_SRC_KHR)
        .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(subresource_range);
    let copy = vk::BufferImageCopy::default()
        .buffer_offset(0)
        // 零值要求 Vulkan 按 format texel 大小紧密排列，不携带原生 row pitch。
        .buffer_row_length(0)
        .buffer_image_height(0)
        .image_subresource(color_subresource_layers())
        .image_offset(vk::Offset3D {
            x: region.x,
            y: region.y,
            z: 0,
        })
        .image_extent(vk::Extent3D {
            width: region.width as u32,
            height: region.height as u32,
            depth: 1,
        });
    let to_present = vk::ImageMemoryBarrier::default()
        .src_access_mask(vk::AccessFlags::TRANSFER_READ)
        .dst_access_mask(vk::AccessFlags::MEMORY_READ)
        .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
        .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(subresource_range);
    let to_host = vk::BufferMemoryBarrier::default()
        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
        .dst_access_mask(vk::AccessFlags::HOST_READ)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .buffer(destination)
        .offset(0)
        .size(vk::WHOLE_SIZE);
    // SAFETY: image/destination/command 同属当前 device，区域已由共享契约验证。
    unsafe {
        device.cmd_pipeline_barrier(
            command,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            std::slice::from_ref(&to_transfer),
        );
        device.cmd_copy_image_to_buffer(
            command,
            image,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            destination,
            std::slice::from_ref(&copy),
        );
        device.cmd_pipeline_barrier(
            command,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::HOST,
            vk::DependencyFlags::empty(),
            &[],
            std::slice::from_ref(&to_host),
            &[],
        );
        device.cmd_pipeline_barrier(
            command,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            std::slice::from_ref(&to_present),
        );
    }
}

fn select_readback_memory_type(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    type_bits: u32,
) -> Result<(u32, bool)> {
    // SAFETY: physical_device 来自当前存活 instance；查询不修改原生状态。
    let properties = unsafe { instance.get_physical_device_memory_properties(physical_device) };
    for required in [
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        vk::MemoryPropertyFlags::HOST_VISIBLE,
    ] {
        for index in 0..properties.memory_type_count {
            let flags = properties.memory_types[index as usize].property_flags;
            if type_bits & (1 << index) != 0 && flags.contains(required) {
                return Ok((
                    index,
                    flags.contains(vk::MemoryPropertyFlags::HOST_COHERENT),
                ));
            }
        }
    }
    Err(Error::new(
        Errc::PlatformError,
        "Vulkan Surface readback has no host-visible staging memory type",
    ))
}

fn normalize_surface_pixels(
    format: vk::Format,
    bytes: &[u8],
    pixel_count: usize,
) -> Result<Vec<u32>> {
    if !supports_surface_readback_format(format) {
        return Err(Error::new(
            Errc::NotImplemented,
            format!(
                "Vulkan Surface readback does not support swapchain format {}",
                format.as_raw(),
            ),
        ));
    }
    if bytes.len() != pixel_count.saturating_mul(4) {
        return Err(invalid_readback(
            "Vulkan Surface readback byte length does not match its pixel count",
        ));
    }
    let mut pixels: Vec<u32> = Vec::new();
    pixels.try_reserve_exact(pixel_count).map_err(|error| {
        Error::new(
            Errc::GraphicsOutOfMemory,
            format!("Vulkan Surface readback pixel allocation failed: {error}"),
        )
    })?;
    let bgra = matches!(
        format,
        vk::Format::B8G8R8A8_UNORM | vk::Format::B8G8R8A8_SRGB
    );

    #[cfg(target_endian = "little")]
    {
        // SAFETY: 上方已校验字节数等于像素数乘四；目标容量充足且与输入不重叠，
        // 复制完成后每个 u32 的全部字节均已初始化，随后才设置长度。
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                pixels.as_mut_ptr().cast::<u8>(),
                bytes.len(),
            );
            pixels.set_len(pixel_count);
        }
        if !bgra {
            for pixel in &mut pixels {
                *pixel = (*pixel & 0xff00_ff00)
                    | ((*pixel & 0x00ff_0000) >> 16)
                    | ((*pixel & 0x0000_00ff) << 16);
            }
        }
        return Ok(pixels);
    }

    #[cfg(target_endian = "big")]
    for texel in bytes.chunks_exact(4) {
        let (red, green, blue, alpha) = if bgra {
            (texel[2], texel[1], texel[0], texel[3])
        } else {
            (texel[0], texel[1], texel[2], texel[3])
        };
        pixels.push(
            (u32::from(alpha) << 24)
                | (u32::from(red) << 16)
                | (u32::from(green) << 8)
                | u32::from(blue),
        );
    }
    #[cfg(target_endian = "big")]
    Ok(pixels)
}

fn invalid_readback(message: &'static str) -> Error {
    Error::new(Errc::InvalidState, message)
}

fn readback_size_overflow() -> Error {
    Error::new(
        Errc::InvalidArgument,
        "Vulkan Surface readback staging size overflows",
    )
}
