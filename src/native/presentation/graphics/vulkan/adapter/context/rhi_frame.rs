//! Vulkan 薄 RHI 的单帧命令录制与 RenderPass 原生资源。
//!
//! platform `RhiPassState` 仍拥有顺序、目标、颜色和几何语义；本模块只管理
//! command buffer、layout barrier、VkRenderPass、Framebuffer 与 descriptor arena。

use std::ptr::NonNull;

use ash::vk;

use crate::core::{Errc, Error, Result};
use crate::platform::presentation::rhi::{
    DrawRasterState, LoadAction, RhiColor, RhiExtent, RhiScissor, TextureHandle,
};

use super::rhi_pipeline::VulkanDescriptorArena;
use super::transfer::find_memory_type;
use super::vk_err;

// 帧内上传切片把一次 Draw 观察到的 Buffer 内容冻结到不可变偏移。
#[derive(Clone, Copy)]
pub(super) struct VulkanUploadSlice {
    pub(super) buffer: vk::Buffer,
    pub(super) offset: vk::DeviceSize,
}

// 持久映射的一块帧内上传内存；只有覆盖其提交的 fence 完成后才复用。
struct VulkanUploadChunk {
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
    mapped: NonNull<u8>,
    capacity: vk::DeviceSize,
    cursor: vk::DeviceSize,
}

// 集中服务 Vertex、Index 与 Uniform 的 Vulkan 延迟执行快照机制。
struct VulkanFrameUploadArena {
    chunks: Vec<VulkanUploadChunk>,
    next_capacity: vk::DeviceSize,
}

impl VulkanFrameUploadArena {
    const INITIAL_CAPACITY: vk::DeviceSize = 256 * 1024;

    const fn new() -> Self {
        Self {
            chunks: Vec::new(),
            next_capacity: Self::INITIAL_CAPACITY,
        }
    }

    // fence 已完成后仅回退游标，复用现有原生分配。
    fn reset(&mut self) {
        for chunk in &mut self.chunks {
            chunk.cursor = 0;
        }
    }

    // 把当前共享 Buffer 内容复制到本帧不可变切片，防止后续上传改写早先 Draw。
    fn upload(
        &mut self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        data: &[u8],
        alignment: vk::DeviceSize,
    ) -> Result<VulkanUploadSlice> {
        let size = data.len() as vk::DeviceSize;
        if size == 0 {
            return Err(invalid_state("Vulkan RHI frame upload must not be empty"));
        }
        let alignment = alignment.max(1);
        for chunk in &mut self.chunks {
            let Some(offset) = align_up(chunk.cursor, alignment) else {
                continue;
            };
            let Some(end) = offset.checked_add(size) else {
                continue;
            };
            if end <= chunk.capacity {
                // SAFETY: offset..end 已证明位于持久映射 chunk 内，源切片同长且不重叠。
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        data.as_ptr(),
                        chunk.mapped.as_ptr().add(offset as usize),
                        data.len(),
                    );
                }
                chunk.cursor = end;
                return Ok(VulkanUploadSlice {
                    buffer: chunk.buffer,
                    offset,
                });
            }
        }

        let required = align_up(size, alignment).ok_or_else(upload_overflow)?;
        let capacity = self.next_capacity.max(required);
        let chunk = create_upload_chunk(instance, physical_device, device, capacity)?;
        self.next_capacity = capacity.saturating_mul(2).max(Self::INITIAL_CAPACITY);
        self.chunks.push(chunk);
        // 新 chunk 的游标为零且容量至少覆盖本次请求，递归只会命中一次。
        self.upload(instance, physical_device, device, data, alignment)
    }

    fn shutdown(&mut self, device: &ash::Device) {
        for chunk in self.chunks.drain(..).rev() {
            // SAFETY: 调用方已等待 device idle 或覆盖这些切片的 frame fence。
            unsafe {
                device.unmap_memory(chunk.memory);
                device.destroy_buffer(chunk.buffer, None);
                device.free_memory(chunk.memory, None);
            }
        }
        self.next_capacity = Self::INITIAL_CAPACITY;
    }
}

// 标识一次原生 pass 写入的是 acquired swapchain image 还是共享纹理资源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum VulkanTargetOwner {
    Surface(usize),
    Texture(TextureHandle),
}

// 保存已经由 VulkanContext 从真实资源解析出的 pass 目标。
#[derive(Clone, Copy)]
pub(super) struct VulkanRhiTarget {
    pub(super) owner: VulkanTargetOwner,
    pub(super) image: vk::Image,
    pub(super) view: vk::ImageView,
    pub(super) format: vk::Format,
    pub(super) extent: RhiExtent,
    pub(super) old_layout: vk::ImageLayout,
    pub(super) final_layout: vk::ImageLayout,
}

// 返回 end_render_pass 已经录制的最终布局事实，供资源 owner 更新跟踪值。
pub(super) struct VulkanCompletedTarget {
    pub(super) owner: VulkanTargetOwner,
    pub(super) layout: vk::ImageLayout,
}

// RenderPass 原生身份由格式与 load operation 共同决定。
struct VulkanRenderPass {
    format: vk::Format,
    clear: bool,
    native: vk::RenderPass,
}

// 保存一个尚未结束的原生 RenderPass。
struct VulkanActivePass {
    target: VulkanRhiTarget,
    render_pass: vk::RenderPass,
}

// 单一 VulkanContext 的一帧录制状态与可复用原生资源。
pub(super) struct VulkanRhiFrame {
    prepared: bool,
    recording: bool,
    active: Option<VulkanActivePass>,
    render_passes: Vec<VulkanRenderPass>,
    retained_framebuffers: Vec<vk::Framebuffer>,
    descriptors: VulkanDescriptorArena,
    uploads: VulkanFrameUploadArena,
}

impl VulkanRhiFrame {
    pub(super) const fn new() -> Self {
        Self {
            prepared: false,
            recording: false,
            active: None,
            render_passes: Vec::new(),
            retained_framebuffers: Vec::new(),
            descriptors: VulkanDescriptorArena::new(),
            uploads: VulkanFrameUploadArena::new(),
        }
    }

    pub(super) const fn is_prepared(&self) -> bool {
        self.prepared
    }

    pub(super) const fn is_recording(&self) -> bool {
        self.recording
    }

    // 上一 frame fence 完成后重置命令与全部单帧资源。
    pub(super) fn prepare(
        &mut self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
    ) -> Result<()> {
        if self.active.is_some() {
            return Err(invalid_state(
                "Vulkan RHI cannot prepare with an open render pass",
            ));
        }
        // SAFETY: 调用方已等待覆盖该 command buffer 的唯一 frame fence。
        unsafe {
            device
                .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
                .map_err(|error| vk_err("vkResetCommandBuffer RHI frame", error))?;
            // reset 已解除旧命令对临时 framebuffer 的引用。
            for framebuffer in self.retained_framebuffers.drain(..).rev() {
                device.destroy_framebuffer(framebuffer, None);
            }
        }
        self.descriptors.reset(device)?;
        self.uploads.reset();
        self.recording = false;
        self.prepared = true;
        Ok(())
    }

    // 在第一条 pass 或 copy 前惰性开始唯一 command buffer。
    pub(super) fn ensure_recording(
        &mut self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
    ) -> Result<()> {
        if self.recording {
            return Ok(());
        }
        if !self.prepared {
            return Err(invalid_state(
                "Vulkan RHI frame command buffer is not prepared",
            ));
        }
        let begin = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        // SAFETY: command buffer 已重置且当前没有其它录制者。
        unsafe {
            device
                .begin_command_buffer(command_buffer, &begin)
                .map_err(|error| vk_err("vkBeginCommandBuffer RHI frame", error))?;
        }
        self.recording = true;
        Ok(())
    }

    // 创建并开始一个已经通过共享状态机验证的原生 render pass。
    pub(super) fn begin_render_pass(
        &mut self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
        target: VulkanRhiTarget,
        load: LoadAction,
    ) -> Result<()> {
        if self.active.is_some() {
            return Err(invalid_state(
                "Vulkan RHI native render pass is already open",
            ));
        }
        self.ensure_recording(device, command_buffer)?;
        let render_pass = self.render_pass(device, target.format, load)?;
        record_image_transition(
            device,
            command_buffer,
            target.image,
            target.old_layout,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        );
        let attachments = [target.view];
        let framebuffer_info = vk::FramebufferCreateInfo::default()
            .render_pass(render_pass)
            .attachments(&attachments)
            .width(target.extent.width)
            .height(target.extent.height)
            .layers(1);
        // SAFETY: view 与 render pass 同属当前 device 且格式兼容。
        let framebuffer = unsafe { device.create_framebuffer(&framebuffer_info, None) }
            .map_err(|error| vk_err("vkCreateFramebuffer RHI pass", error))?;
        self.retained_framebuffers.push(framebuffer);

        let clear_values = match load {
            LoadAction::Clear(color) => vec![vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: color.components(),
                },
            }],
            LoadAction::Load => Vec::new(),
        };
        let begin_info = vk::RenderPassBeginInfo::default()
            .render_pass(render_pass)
            .framebuffer(framebuffer)
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: target.extent.width,
                    height: target.extent.height,
                },
            })
            .clear_values(&clear_values);
        // SAFETY: framebuffer、render pass 与 command buffer 存活且当前位于录制态。
        unsafe {
            device.cmd_begin_render_pass(command_buffer, &begin_info, vk::SubpassContents::INLINE);
        }
        self.active = Some(VulkanActivePass {
            target,
            render_pass,
        });
        Ok(())
    }

    // 结束当前 pass 并把目标转换到 Surface present 或 texture sampled 布局。
    pub(super) fn end_render_pass(
        &mut self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
    ) -> Result<VulkanCompletedTarget> {
        let active = self
            .active
            .take()
            .ok_or_else(|| invalid_state("Vulkan RHI native render pass is not open"))?;
        // SAFETY: active 只在成功 cmd_begin_render_pass 后建立。
        unsafe { device.cmd_end_render_pass(command_buffer) };
        record_image_transition(
            device,
            command_buffer,
            active.target.image,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            active.target.final_layout,
        );
        Ok(VulkanCompletedTarget {
            owner: active.target.owner,
            layout: active.target.final_layout,
        })
    }

    // 返回当前 pass 的兼容 render pass 与目标格式，供 Pipeline 按需物化。
    pub(super) fn pipeline_target(&self) -> Result<(vk::Format, vk::RenderPass)> {
        let active = self
            .active
            .as_ref()
            .ok_or_else(|| invalid_state("Vulkan RHI draw has no active render pass"))?;
        Ok((active.target.format, active.render_pass))
    }

    // 为当前 DrawPacket 分配一个单帧 descriptor set。
    pub(super) fn allocate_descriptor_set(
        &mut self,
        device: &ash::Device,
        layout: vk::DescriptorSetLayout,
    ) -> Result<vk::DescriptorSet> {
        if self.active.is_none() {
            return Err(invalid_state(
                "Vulkan RHI descriptor allocation requires a render pass",
            ));
        }
        self.descriptors.allocate(device, layout)
    }

    // 为一次 Draw 冻结共享 Buffer 当前内容，三类绑定复用同一个上传 arena。
    pub(super) fn upload_buffer_snapshot(
        &mut self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        data: &[u8],
        alignment: vk::DeviceSize,
    ) -> Result<VulkanUploadSlice> {
        if self.active.is_none() {
            return Err(invalid_state(
                "Vulkan RHI buffer snapshot requires a render pass",
            ));
        }
        self.uploads
            .upload(instance, physical_device, device, data, alignment)
    }

    // 机械编码共享 viewport 与左上原点 scissor。
    pub(super) fn apply_raster(
        &self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
        raster: DrawRasterState,
        extent: RhiExtent,
    ) {
        let viewport = raster.viewport();
        let native_viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: viewport.width,
            height: viewport.height,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        let scissor = raster.scissor().unwrap_or(RhiScissor {
            x: 0,
            y: 0,
            width: extent.width as i32,
            height: extent.height as i32,
        });
        let native_scissor = vk::Rect2D {
            offset: vk::Offset2D {
                x: scissor.x,
                y: scissor.y,
            },
            extent: vk::Extent2D {
                width: scissor.width as u32,
                height: scissor.height as u32,
            },
        };
        // SAFETY: 两个动态状态均由共享 pass 状态证明落在当前目标内。
        unsafe {
            device.cmd_set_viewport(command_buffer, 0, std::slice::from_ref(&native_viewport));
            device.cmd_set_scissor(command_buffer, 0, std::slice::from_ref(&native_scissor));
        }
    }

    // 使用 Vulkan 原生 clear attachment 表达共享局部清理原语。
    pub(super) fn clear_rect(
        &self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
        color: RhiColor,
        scissor: RhiScissor,
    ) -> Result<()> {
        if self.active.is_none() {
            return Err(invalid_state("Vulkan RHI clear requires a render pass"));
        }
        let attachment = vk::ClearAttachment::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .color_attachment(0)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: color.components(),
                },
            });
        let rect = vk::ClearRect::default()
            .rect(vk::Rect2D {
                offset: vk::Offset2D {
                    x: scissor.x,
                    y: scissor.y,
                },
                extent: vk::Extent2D {
                    width: scissor.width as u32,
                    height: scissor.height as u32,
                },
            })
            .base_array_layer(0)
            .layer_count(1);
        // SAFETY: pass 与区域均由共享状态机验证，attachment 0 是唯一颜色目标。
        unsafe {
            device.cmd_clear_attachments(
                command_buffer,
                std::slice::from_ref(&attachment),
                std::slice::from_ref(&rect),
            );
        }
        Ok(())
    }

    // 结束 command buffer，调用方随后执行唯一 queue submit。
    pub(super) fn finish_recording(
        &mut self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
    ) -> Result<()> {
        if self.active.is_some() {
            return Err(invalid_state(
                "Vulkan RHI cannot submit with an open render pass",
            ));
        }
        self.ensure_recording(device, command_buffer)?;
        // SAFETY: command buffer 当前处于本 owner 的录制态且没有活动 pass。
        unsafe {
            device
                .end_command_buffer(command_buffer)
                .map_err(|error| vk_err("vkEndCommandBuffer RHI frame", error))?;
        }
        self.recording = false;
        Ok(())
    }

    pub(super) fn mark_submitted(&mut self) {
        self.prepared = false;
        self.recording = false;
    }

    pub(super) fn shutdown(&mut self, device: &ash::Device) {
        self.active = None;
        self.recording = false;
        self.prepared = false;
        // SAFETY: Context shutdown 已等待 device idle，所有原生 child 不再被引用。
        unsafe {
            for framebuffer in self.retained_framebuffers.drain(..).rev() {
                device.destroy_framebuffer(framebuffer, None);
            }
            for render_pass in self.render_passes.drain(..).rev() {
                device.destroy_render_pass(render_pass.native, None);
            }
        }
        self.descriptors.shutdown(device);
        self.uploads.shutdown(device);
    }

    fn render_pass(
        &mut self,
        device: &ash::Device,
        format: vk::Format,
        load: LoadAction,
    ) -> Result<vk::RenderPass> {
        let clear = matches!(load, LoadAction::Clear(_));
        if let Some(pass) = self
            .render_passes
            .iter()
            .find(|pass| pass.format == format && pass.clear == clear)
        {
            return Ok(pass.native);
        }
        let attachment = vk::AttachmentDescription::default()
            .format(format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(if clear {
                vk::AttachmentLoadOp::CLEAR
            } else {
                vk::AttachmentLoadOp::LOAD
            })
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        let color_reference = vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        };
        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&color_reference));
        let dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            );
        let create_info = vk::RenderPassCreateInfo::default()
            .attachments(std::slice::from_ref(&attachment))
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(std::slice::from_ref(&dependency));
        // SAFETY: create_info 只引用同步调用期间存活的单附件描述。
        let native = unsafe { device.create_render_pass(&create_info, None) }
            .map_err(|error| vk_err("vkCreateRenderPass RHI", error))?;
        self.render_passes.push(VulkanRenderPass {
            format,
            clear,
            native,
        });
        Ok(native)
    }
}

fn create_upload_chunk(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    capacity: vk::DeviceSize,
) -> Result<VulkanUploadChunk> {
    let create_info = vk::BufferCreateInfo::default()
        .size(capacity)
        .usage(
            vk::BufferUsageFlags::VERTEX_BUFFER
                | vk::BufferUsageFlags::INDEX_BUFFER
                | vk::BufferUsageFlags::UNIFORM_BUFFER,
        )
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    // SAFETY: capacity 非零且用途是 frame upload arena 的封闭集合。
    let buffer = unsafe { device.create_buffer(&create_info, None) }
        .map_err(|error| vk_err("vkCreateBuffer RHI frame upload", error))?;
    // SAFETY: buffer 刚由当前 device 创建且仍存活。
    let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let memory_type_index = match find_memory_type(
        instance,
        physical_device,
        requirements.memory_type_bits,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        "host-visible coherent RHI frame upload",
    ) {
        Ok(index) => index,
        Err(error) => {
            // SAFETY: buffer 尚未绑定内存或进入 arena。
            unsafe { device.destroy_buffer(buffer, None) };
            return Err(error);
        }
    };
    let allocate_info = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type_index);
    // SAFETY: 内存类型来自同一 physical device 的 buffer requirements。
    let memory = match unsafe { device.allocate_memory(&allocate_info, None) } {
        Ok(memory) => memory,
        Err(error) => {
            // SAFETY: 分配失败后 buffer 尚未绑定或登记。
            unsafe { device.destroy_buffer(buffer, None) };
            return Err(vk_err("vkAllocateMemory RHI frame upload", error));
        }
    };
    // SAFETY: buffer 与 memory 同属当前 device，分配容量满足 requirements。
    if let Err(error) = unsafe { device.bind_buffer_memory(buffer, memory, 0) } {
        // SAFETY: 绑定失败路径按创建逆序释放唯一对象。
        unsafe {
            device.free_memory(memory, None);
            device.destroy_buffer(buffer, None);
        }
        return Err(vk_err("vkBindBufferMemory RHI frame upload", error));
    }
    // SAFETY: memory 是 HOST_VISIBLE，映射范围覆盖整个 allocation。
    let mapped = match unsafe {
        device.map_memory(memory, 0, requirements.size, vk::MemoryMapFlags::empty())
    } {
        Ok(mapped) => match NonNull::new(mapped.cast::<u8>()) {
            Some(mapped) => Ok(mapped),
            None => {
                // SAFETY: vkMapMemory 已返回成功，失败发布前先结束映射生命周期。
                unsafe { device.unmap_memory(memory) };
                Err(invalid_state(
                    "Vulkan RHI frame upload mapping returned a null pointer",
                ))
            }
        },
        Err(error) => Err(vk_err("vkMapMemory RHI frame upload", error)),
    };
    let mapped = match mapped {
        Ok(mapped) => mapped,
        Err(error) => {
            // SAFETY: 映射失败后没有 CPU 借用，按创建逆序释放。
            unsafe {
                device.destroy_buffer(buffer, None);
                device.free_memory(memory, None);
            }
            return Err(error);
        }
    };
    Ok(VulkanUploadChunk {
        buffer,
        memory,
        mapped,
        capacity,
        cursor: 0,
    })
}

fn align_up(value: vk::DeviceSize, alignment: vk::DeviceSize) -> Option<vk::DeviceSize> {
    let remainder = value % alignment;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(alignment - remainder)
    }
}

fn upload_overflow() -> Error {
    Error::new(
        Errc::GraphicsOutOfMemory,
        "Vulkan RHI frame upload size exceeds the addressable range",
    )
}

// 记录颜色 image 的显式布局与可见性转换。
fn record_image_transition(
    device: &ash::Device,
    command_buffer: vk::CommandBuffer,
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    if old_layout == new_layout {
        return;
    }
    let (source_stage, source_access) = source_scope(old_layout);
    let (destination_stage, destination_access) = destination_scope(new_layout);
    let barrier = vk::ImageMemoryBarrier::default()
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(super::super::rhi::color_subresource_range())
        .src_access_mask(source_access)
        .dst_access_mask(destination_access);
    // SAFETY: image 与 command buffer 同属当前 device，布局由唯一资源 owner 跟踪。
    unsafe {
        device.cmd_pipeline_barrier(
            command_buffer,
            source_stage,
            destination_stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            std::slice::from_ref(&barrier),
        );
    }
}

fn source_scope(layout: vk::ImageLayout) -> (vk::PipelineStageFlags, vk::AccessFlags) {
    match layout {
        vk::ImageLayout::UNDEFINED => (
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::AccessFlags::empty(),
        ),
        vk::ImageLayout::PRESENT_SRC_KHR => (
            // reacquire 后的首个布局转换必须位于 acquire semaphore 的等待范围内。
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::AccessFlags::MEMORY_READ,
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

fn destination_scope(layout: vk::ImageLayout) -> (vk::PipelineStageFlags, vk::AccessFlags) {
    match layout {
        vk::ImageLayout::PRESENT_SRC_KHR => (
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::AccessFlags::MEMORY_READ,
        ),
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => (
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::AccessFlags::SHADER_READ,
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

fn invalid_state(message: &'static str) -> Error {
    Error::new(Errc::InvalidState, message)
}
