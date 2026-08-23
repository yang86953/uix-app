//! Vulkan RHI Texture、布局转换与串行即时提交实现。

use std::cell::Cell;
use std::ptr;

use ash::vk;

use crate::core::{Errc, Error, Result};
use crate::platform::presentation::rhi::{
    RhiTextureResource, RhiTextureTransferBounds, TextureDesc, ValidatedRhiTextureUpload,
};

use super::super::rhi::{color_subresource_layers, color_subresource_range, texture_format};
use super::transfer::find_memory_type;
use super::vk_err;

// 保存一个 Vulkan Image 的完整 owner 与共享描述。
pub(super) struct VulkanRhiTexture {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
    desc: TextureDesc,
    layout: Cell<vk::ImageLayout>,
}

impl RhiTextureResource for VulkanRhiTexture {
    fn desc(&self) -> TextureDesc {
        self.desc
    }
}

impl VulkanRhiTexture {
    pub(super) const fn image(&self) -> vk::Image {
        self.image
    }

    pub(super) const fn view(&self) -> vk::ImageView {
        self.view
    }

    pub(super) fn layout(&self) -> vk::ImageLayout {
        self.layout.get()
    }

    pub(super) fn set_layout(&self, layout: vk::ImageLayout) {
        self.layout.set(layout);
    }
}

// 保存只在一次纹理上传期间存活的 HOST_COHERENT staging Buffer。
struct VulkanTextureStaging {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
}

// 保存 Vulkan RHI 资源上传与复制共用的 owner-thread 串行命令池。
pub(super) struct VulkanImmediateCommands {
    pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
}

impl VulkanImmediateCommands {
    pub(super) const fn new() -> Self {
        Self {
            pool: vk::CommandPool::null(),
            command_buffer: vk::CommandBuffer::null(),
        }
    }

    fn ensure(&mut self, device: &ash::Device, queue_family_index: u32) -> Result<()> {
        if self.pool != vk::CommandPool::null() {
            return Ok(());
        }
        let create_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family_index)
            .flags(vk::CommandPoolCreateFlags::TRANSIENT);
        // SAFETY: queue family 来自创建当前 graphics+present queue 的同一选择结果。
        let pool = unsafe { device.create_command_pool(&create_info, None) }
            .map_err(|error| vk_err("vkCreateCommandPool RHI immediate", error))?;
        let allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        // SAFETY: pool 刚创建且仍存活，描述只申请一个 primary command buffer。
        let command_buffer = match unsafe { device.allocate_command_buffers(&allocate_info) } {
            Ok(mut buffers) => buffers.pop(),
            Err(error) => {
                // SAFETY: pool 尚未移交正式 owner，销毁会一并释放已分配命令。
                unsafe { device.destroy_command_pool(pool, None) };
                return Err(vk_err("vkAllocateCommandBuffers RHI immediate", error));
            }
        }
        .ok_or_else(|| {
            // SAFETY: 空返回时 pool 尚未移交，立即回收。
            unsafe { device.destroy_command_pool(pool, None) };
            Error::new(
                Errc::PlatformError,
                "Vulkan RHI immediate command allocation returned no buffer",
            )
        })?;
        self.pool = pool;
        self.command_buffer = command_buffer;
        Ok(())
    }

    pub(super) fn execute<F>(
        &mut self,
        device: &ash::Device,
        queue: vk::Queue,
        queue_family_index: u32,
        record: F,
    ) -> Result<()>
    where
        F: FnOnce(vk::CommandBuffer),
    {
        self.ensure(device, queue_family_index)?;
        // SAFETY: 上一次调用返回前已等待同一 queue idle，pool 中没有在途命令。
        unsafe {
            device
                .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
                .map_err(|error| vk_err("vkResetCommandPool RHI immediate", error))?;
            let begin = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            device
                .begin_command_buffer(self.command_buffer, &begin)
                .map_err(|error| vk_err("vkBeginCommandBuffer RHI immediate", error))?;
            record(self.command_buffer);
            device
                .end_command_buffer(self.command_buffer)
                .map_err(|error| vk_err("vkEndCommandBuffer RHI immediate", error))?;
            let submit = vk::SubmitInfo::default()
                .command_buffers(std::slice::from_ref(&self.command_buffer));
            device
                .queue_submit(queue, std::slice::from_ref(&submit), vk::Fence::null())
                .map_err(|error| vk_err("vkQueueSubmit RHI immediate", error))?;
            device
                .queue_wait_idle(queue)
                .map_err(|error| vk_err("vkQueueWaitIdle RHI immediate", error))?;
        }
        Ok(())
    }

    pub(super) fn shutdown(&mut self, device: &ash::Device) {
        if self.pool != vk::CommandPool::null() {
            // SAFETY: Context shutdown 已等待 device idle，销毁 pool 会释放唯一 command buffer。
            unsafe { device.destroy_command_pool(self.pool, None) };
            self.pool = vk::CommandPool::null();
            self.command_buffer = vk::CommandBuffer::null();
        }
    }
}

pub(super) fn create_texture(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    desc: TextureDesc,
) -> Result<VulkanRhiTexture> {
    let native_desc = desc.validate()?;
    let (width, height) = native_desc.size_i32();
    let mut usage = vk::ImageUsageFlags::SAMPLED
        | vk::ImageUsageFlags::TRANSFER_SRC
        | vk::ImageUsageFlags::TRANSFER_DST;
    if desc.format().supports_render_target() {
        usage |= vk::ImageUsageFlags::COLOR_ATTACHMENT;
    }
    let create_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(texture_format(desc.format()))
        .extent(vk::Extent3D {
            width: width as u32,
            height: height as u32,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    // SAFETY: 描述由 platform TextureDesc 共同值域机械投影。
    let image = unsafe { device.create_image(&create_info, None) }
        .map_err(|error| vk_err("vkCreateImage RHI texture", error))?;
    // SAFETY: image 为当前 device 刚创建且仍存活的对象。
    let requirements = unsafe { device.get_image_memory_requirements(image) };
    let memory_type_index = match find_memory_type(
        instance,
        physical_device,
        requirements.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        "device-local RHI texture",
    ) {
        Ok(index) => index,
        Err(error) => {
            // SAFETY: image 尚未登记且未绑定内存。
            unsafe { device.destroy_image(image, None) };
            return Err(error);
        }
    };
    let allocate_info = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type_index);
    // SAFETY: 内存类型来自当前 image requirements 的合法交集。
    let memory = match unsafe { device.allocate_memory(&allocate_info, None) } {
        Ok(memory) => memory,
        Err(error) => {
            // SAFETY: image 尚未登记且失败后不再使用。
            unsafe { device.destroy_image(image, None) };
            return Err(vk_err("vkAllocateMemory RHI texture", error));
        }
    };
    // SAFETY: image 与 memory 同属当前 device 且 allocation 满足 requirements。
    if let Err(error) = unsafe { device.bind_image_memory(image, memory, 0) } {
        // SAFETY: 两个对象尚未登记，按创建逆序释放。
        unsafe {
            device.free_memory(memory, None);
            device.destroy_image(image, None);
        }
        return Err(vk_err("vkBindImageMemory RHI texture", error));
    }
    let view_info = vk::ImageViewCreateInfo::default()
        .image(image)
        .view_type(vk::ImageViewType::TYPE_2D)
        .format(texture_format(desc.format()))
        .subresource_range(color_subresource_range());
    // SAFETY: image 已绑定内存，view 描述与 image 格式和子资源一致。
    let view = match unsafe { device.create_image_view(&view_info, None) } {
        Ok(view) => view,
        Err(error) => {
            // SAFETY: 失败资源尚未登记，先删 image 再释放绑定内存。
            unsafe {
                device.destroy_image(image, None);
                device.free_memory(memory, None);
            }
            return Err(vk_err("vkCreateImageView RHI texture", error));
        }
    };
    Ok(VulkanRhiTexture {
        image,
        memory,
        view,
        desc,
        layout: Cell::new(vk::ImageLayout::UNDEFINED),
    })
}

pub(super) fn destroy_texture(device: &ash::Device, texture: VulkanRhiTexture) {
    // SAFETY: 调用方从唯一资源表取出对象并保证没有在途引用。
    unsafe {
        device.destroy_image_view(texture.view, None);
        device.destroy_image(texture.image, None);
        device.free_memory(texture.memory, None);
    }
}

pub(super) fn update_texture(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    queue: vk::Queue,
    queue_family_index: u32,
    immediate: &mut VulkanImmediateCommands,
    texture: &VulkanRhiTexture,
    upload: ValidatedRhiTextureUpload<'_>,
) -> Result<()> {
    let staging = create_staging(instance, physical_device, device, upload.data())?;
    let old_layout = texture.layout.get();
    // Vulkan copy 需要“原点 + 尺寸”，不能把共享矩形的 right/bottom 当成宽高。
    let ((x, y), (width, height)) = upload.bounds().native_origin_and_size_i32();
    let copy = vk::BufferImageCopy::default()
        .buffer_offset(0)
        .buffer_row_length(0)
        .buffer_image_height(0)
        .image_subresource(color_subresource_layers())
        .image_offset(vk::Offset3D { x, y, z: 0 })
        .image_extent(vk::Extent3D {
            width: width as u32,
            height: height as u32,
            depth: 1,
        });
    let result = immediate.execute(device, queue, queue_family_index, |command| {
        transition_image(
            device,
            command,
            texture.image,
            old_layout,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );
        // SAFETY: staging 与 texture 存活；区域和载荷已由 platform 验证。
        unsafe {
            device.cmd_copy_buffer_to_image(
                command,
                staging.buffer,
                texture.image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                std::slice::from_ref(&copy),
            );
        }
        transition_image(
            device,
            command,
            texture.image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );
    });
    destroy_staging(device, staging);
    if result.is_ok() {
        texture
            .layout
            .set(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    }
    result
}

pub(super) fn copy_texture(
    device: &ash::Device,
    queue: vk::Queue,
    queue_family_index: u32,
    immediate: &mut VulkanImmediateCommands,
    source: &VulkanRhiTexture,
    destination: &VulkanRhiTexture,
    bounds: RhiTextureTransferBounds,
) -> Result<()> {
    immediate.execute(device, queue, queue_family_index, |command| {
        record_texture_copy_commands(device, command, source, destination, bounds);
    })?;
    source.layout.set(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    destination
        .layout
        .set(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    Ok(())
}

// 把 FramePlan texture copy 录入当前帧 command buffer，保持与 pass 的总顺序。
pub(super) fn record_texture_copy(
    device: &ash::Device,
    command: vk::CommandBuffer,
    source: &VulkanRhiTexture,
    destination: &VulkanRhiTexture,
    bounds: RhiTextureTransferBounds,
) {
    record_texture_copy_commands(device, command, source, destination, bounds);
    source.layout.set(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    destination
        .layout
        .set(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
}

fn record_texture_copy_commands(
    device: &ash::Device,
    command: vk::CommandBuffer,
    source: &VulkanRhiTexture,
    destination: &VulkanRhiTexture,
    bounds: RhiTextureTransferBounds,
) {
    let source_layout = source.layout.get();
    let destination_layout = destination.layout.get();
    // VkImageCopy 两端同样消费原点和传输尺寸，禁止把远端边界误作 extent。
    let ((source_x, source_y), (width, height)) = bounds.source().native_origin_and_size_i32();
    let ((destination_x, destination_y), _) = bounds.destination().native_origin_and_size_i32();
    let region = vk::ImageCopy::default()
        .src_subresource(color_subresource_layers())
        .src_offset(vk::Offset3D {
            x: source_x,
            y: source_y,
            z: 0,
        })
        .dst_subresource(color_subresource_layers())
        .dst_offset(vk::Offset3D {
            x: destination_x,
            y: destination_y,
            z: 0,
        })
        .extent(vk::Extent3D {
            width: width as u32,
            height: height as u32,
            depth: 1,
        });
    transition_image(
        device,
        command,
        source.image,
        source_layout,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
    );
    transition_image(
        device,
        command,
        destination.image,
        destination_layout,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
    );
    // SAFETY: 源与目标是不同资源，格式和范围已由 platform TextureCopy 验证。
    unsafe {
        device.cmd_copy_image(
            command,
            source.image,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            destination.image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            std::slice::from_ref(&region),
        );
    }
    transition_image(
        device,
        command,
        source.image,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    );
    transition_image(
        device,
        command,
        destination.image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    );
}

fn create_staging(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    data: &[u8],
) -> Result<VulkanTextureStaging> {
    let create_info = vk::BufferCreateInfo::default()
        .size(data.len() as u64)
        .usage(vk::BufferUsageFlags::TRANSFER_SRC)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    // SAFETY: 非空载荷长度已由 ValidatedRhiTextureUpload 证明。
    let buffer = unsafe { device.create_buffer(&create_info, None) }
        .map_err(|error| vk_err("vkCreateBuffer RHI texture staging", error))?;
    // SAFETY: buffer 为刚创建且仍存活的对象。
    let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let memory_type_index = match find_memory_type(
        instance,
        physical_device,
        requirements.memory_type_bits,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        "host-visible coherent RHI texture staging",
    ) {
        Ok(index) => index,
        Err(error) => {
            // SAFETY: buffer 尚未绑定或登记。
            unsafe { device.destroy_buffer(buffer, None) };
            return Err(error);
        }
    };
    let allocate_info = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type_index);
    // SAFETY: memory type 与 buffer requirements 匹配。
    let memory = match unsafe { device.allocate_memory(&allocate_info, None) } {
        Ok(memory) => memory,
        Err(error) => {
            // SAFETY: buffer 尚未登记且分配失败后不再使用。
            unsafe { device.destroy_buffer(buffer, None) };
            return Err(vk_err("vkAllocateMemory RHI texture staging", error));
        }
    };
    // SAFETY: buffer 与 memory 同属当前 device 且容量满足 requirements。
    if let Err(error) = unsafe { device.bind_buffer_memory(buffer, memory, 0) } {
        // SAFETY: 绑定失败对象尚未登记，按创建逆序释放。
        unsafe {
            device.free_memory(memory, None);
            device.destroy_buffer(buffer, None);
        }
        return Err(vk_err("vkBindBufferMemory RHI texture staging", error));
    }
    // SAFETY: HOST_VISIBLE 映射范围覆盖完整已验证载荷。
    let mapped = match unsafe {
        device.map_memory(memory, 0, data.len() as u64, vk::MemoryMapFlags::empty())
    } {
        Ok(mapped) => mapped,
        Err(error) => {
            // SAFETY: 尚未提交的 staging 对象由本函数唯一拥有。
            unsafe {
                device.destroy_buffer(buffer, None);
                device.free_memory(memory, None);
            }
            return Err(vk_err("vkMapMemory RHI texture staging", error));
        }
    };
    // SAFETY: mapped 与 data 均至少包含 data.len() 字节且不重叠。
    unsafe {
        ptr::copy_nonoverlapping(data.as_ptr(), mapped.cast(), data.len());
        device.unmap_memory(memory);
    }
    Ok(VulkanTextureStaging { buffer, memory })
}

fn destroy_staging(device: &ash::Device, staging: VulkanTextureStaging) {
    // SAFETY: immediate execute 返回前已等待 queue idle，staging 不再被命令引用。
    unsafe {
        device.destroy_buffer(staging.buffer, None);
        device.free_memory(staging.memory, None);
    }
}

fn transition_image(
    device: &ash::Device,
    command: vk::CommandBuffer,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    if old_layout == new_layout {
        return;
    }
    let (source_stage, source_access) = layout_source(old_layout);
    let (destination_stage, destination_access) = layout_destination(new_layout);
    let barrier = vk::ImageMemoryBarrier::default()
        .src_access_mask(source_access)
        .dst_access_mask(destination_access)
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(color_subresource_range());
    // SAFETY: command 正在记录；image 存活；布局状态由唯一 Cell owner 串行维护。
    unsafe {
        device.cmd_pipeline_barrier(
            command,
            source_stage,
            destination_stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            std::slice::from_ref(&barrier),
        );
    }
}

fn layout_source(layout: vk::ImageLayout) -> (vk::PipelineStageFlags, vk::AccessFlags) {
    match layout {
        vk::ImageLayout::UNDEFINED => (
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::AccessFlags::empty(),
        ),
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => (
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::AccessFlags::SHADER_READ,
        ),
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL => (
            vk::PipelineStageFlags::TRANSFER,
            vk::AccessFlags::TRANSFER_READ,
        ),
        vk::ImageLayout::TRANSFER_DST_OPTIMAL => (
            vk::PipelineStageFlags::TRANSFER,
            vk::AccessFlags::TRANSFER_WRITE,
        ),
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => (
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        ),
        _ => (
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
        ),
    }
}

fn layout_destination(layout: vk::ImageLayout) -> (vk::PipelineStageFlags, vk::AccessFlags) {
    match layout {
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => (
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::AccessFlags::SHADER_READ,
        ),
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL => (
            vk::PipelineStageFlags::TRANSFER,
            vk::AccessFlags::TRANSFER_READ,
        ),
        vk::ImageLayout::TRANSFER_DST_OPTIMAL => (
            vk::PipelineStageFlags::TRANSFER,
            vk::AccessFlags::TRANSFER_WRITE,
        ),
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => (
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        ),
        _ => (
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
        ),
    }
}
