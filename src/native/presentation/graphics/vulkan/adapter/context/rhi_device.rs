//! Vulkan `GraphicsDevice` 资源生命周期与阶段性命令入口。
//!
//! 资源身份、描述验证和陈旧句柄语义全部由 platform RHI 资源表拥有；本模块
//! 只持有 Vulkan 对象并执行 Vulkan 内存、上传与销毁操作。

use std::ptr;

use ash::vk;

use crate::core::{Errc, Error, Result};
use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, BufferUsage, DrawPacket, GraphicsDevice, GraphicsDeviceCapabilities,
    LoadAction, PipelineBinding, PipelineDesc, RenderTargetHandle, RhiBufferResource,
    RhiBufferResourceTable, RhiBufferUpload, RhiBufferUploadPreflight, RhiColor, RhiPassState,
    RhiPipelineResourceTable, RhiPresentTransaction, RhiResourceTable, RhiScissor,
    RhiSubmissionSequence, RhiTextureResource, RhiTextureResourceTable, RhiTextureUpload,
    SamplerDesc, SamplerHandle, SubmissionHandle, SurfaceToken, TextureCopy, TextureDesc,
    TextureHandle, TextureMove, ValidatedRhiPresent,
};

use super::super::rhi::{VulkanSamplerState, index_type, texture_format};
use super::rhi_frame::{VulkanCompletedTarget, VulkanRhiFrame, VulkanRhiTarget, VulkanTargetOwner};
use super::rhi_pipeline::VulkanRhiPipeline;
use super::rhi_texture::{
    VulkanImmediateCommands, VulkanRhiTexture, create_texture, destroy_texture,
    record_texture_copy, update_texture,
};
use super::transfer::find_memory_type;
use super::{VulkanContext, vk_err};

// 保存一个 Vulkan Buffer 及其由 platform 契约冻结的描述。
struct VulkanRhiBuffer {
    native: vk::Buffer,
    memory: vk::DeviceMemory,
    desc: BufferDesc,
    // 保存共享 Buffer 的最新逻辑内容，供延迟执行的 Draw 冻结帧内快照。
    shadow: Vec<u8>,
}

impl RhiBufferResource for VulkanRhiBuffer {
    fn desc(&self) -> BufferDesc {
        self.desc
    }
}

// 保存一个 Vulkan Sampler 及其共享描述，供后续 Draw 资源预检使用。
struct VulkanRhiSampler {
    native: vk::Sampler,
    desc: SamplerDesc,
}

// Vulkan context 唯一拥有的 RHI 资源状态。
pub(super) struct VulkanRhiDevice {
    buffers: RhiBufferResourceTable<VulkanRhiBuffer>,
    textures: RhiTextureResourceTable<VulkanRhiTexture>,
    samplers: RhiResourceTable<SamplerHandle, VulkanRhiSampler>,
    pipelines: RhiPipelineResourceTable<VulkanRhiPipeline>,
    pass: RhiPassState,
    submissions: RhiSubmissionSequence,
    frame: VulkanRhiFrame,
    immediate: VulkanImmediateCommands,
    // 保存录制 TextureMove 后必须活到覆盖提交完成的临时纹理。
    texture_move_scratch_after_submit: Vec<TextureHandle>,
    uniform_alignment: vk::DeviceSize,
}

impl VulkanRhiDevice {
    pub(super) const fn new(uniform_alignment: vk::DeviceSize) -> Self {
        Self {
            buffers: RhiBufferResourceTable::new(),
            textures: RhiTextureResourceTable::new(),
            samplers: RhiResourceTable::new(),
            pipelines: RhiPipelineResourceTable::new(),
            pass: RhiPassState::new(),
            submissions: RhiSubmissionSequence::new(),
            frame: VulkanRhiFrame::new(),
            immediate: VulkanImmediateCommands::new(),
            texture_move_scratch_after_submit: Vec::new(),
            uniform_alignment,
        }
    }

    fn create_buffer(
        &mut self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        desc: BufferDesc,
    ) -> Result<BufferHandle> {
        let native_desc = desc.validate()?;
        let shadow_size = native_desc.size_bytes_u32() as usize;
        let mut shadow = Vec::new();
        shadow
            .try_reserve_exact(shadow_size)
            .map_err(|_| buffer_shadow_oom())?;
        shadow.resize(shadow_size, 0);
        let usage = match desc.usage() {
            BufferUsage::Vertex => vk::BufferUsageFlags::VERTEX_BUFFER,
            BufferUsage::Index => vk::BufferUsageFlags::INDEX_BUFFER,
            BufferUsage::Uniform => vk::BufferUsageFlags::UNIFORM_BUFFER,
        };
        let create_info = vk::BufferCreateInfo::default()
            .size(u64::from(native_desc.size_bytes_u32()))
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        // SAFETY: device 存活；创建描述只含经过 platform 共同值域校验的容量和用途。
        let native = unsafe { device.create_buffer(&create_info, None) }
            .map_err(|error| vk_err("vkCreateBuffer RHI", error))?;
        // SAFETY: native 是当前 device 刚创建且仍存活的 Buffer。
        let requirements = unsafe { device.get_buffer_memory_requirements(native) };
        let memory_type_index = match find_memory_type(
            instance,
            physical_device,
            requirements.memory_type_bits,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            "host-visible coherent RHI buffer",
        ) {
            Ok(index) => index,
            Err(error) => {
                // SAFETY: Buffer 尚未进入资源表且没有绑定内存或提交引用。
                unsafe { device.destroy_buffer(native, None) };
                return Err(error);
            }
        };
        let allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type_index);
        // SAFETY: 内存类型来自同一 physical device 的需求交集。
        let memory = match unsafe { device.allocate_memory(&allocate_info, None) } {
            Ok(memory) => memory,
            Err(error) => {
                // SAFETY: Buffer 尚未登记且分配失败后不会再使用。
                unsafe { device.destroy_buffer(native, None) };
                return Err(vk_err("vkAllocateMemory RHI buffer", error));
            }
        };
        // SAFETY: native 与 memory 同属当前 device，容量满足查询所得 requirements。
        if let Err(error) = unsafe { device.bind_buffer_memory(native, memory, 0) } {
            // SAFETY: 绑定失败的两个对象尚未登记，按创建逆序释放。
            unsafe {
                device.free_memory(memory, None);
                device.destroy_buffer(native, None);
            }
            return Err(vk_err("vkBindBufferMemory RHI", error));
        }
        Ok(self.buffers.insert(VulkanRhiBuffer {
            native,
            memory,
            desc,
            shadow,
        }))
    }

    fn update_buffer(&mut self, device: &ash::Device, upload: RhiBufferUpload<'_>) -> Result<()> {
        let resource = self.buffers.get_mut(upload.buffer())?;
        let validated = upload.validate(resource.desc)?;
        let size = u64::from(validated.size_bytes_u32());
        resource.shadow[..size as usize].copy_from_slice(validated.data());
        // SAFETY: memory 为 HOST_VISIBLE；范围已由共享描述验证且从偏移零开始。
        let mapped =
            unsafe { device.map_memory(resource.memory, 0, size, vk::MemoryMapFlags::empty()) }
                .map_err(|error| vk_err("vkMapMemory RHI buffer", error))?;
        // SAFETY: mapped 指向至少 size 字节可写范围，源切片同长且二者不重叠。
        unsafe {
            ptr::copy_nonoverlapping(validated.data().as_ptr(), mapped.cast(), size as usize);
            // HOST_COHERENT 资源不需要显式 flush；本次映射由当前调用唯一拥有。
            device.unmap_memory(resource.memory);
        }
        Ok(())
    }

    fn create_sampler(&mut self, device: &ash::Device, desc: SamplerDesc) -> Result<SamplerHandle> {
        let state = VulkanSamplerState::from_desc(desc);
        let create_info = vk::SamplerCreateInfo::default()
            .min_filter(state.min_filter)
            .mag_filter(state.mag_filter)
            .mipmap_mode(state.mipmap_mode)
            .address_mode_u(state.address_mode_u)
            .address_mode_v(state.address_mode_v)
            .address_mode_w(state.address_mode_w)
            .mip_lod_bias(0.0)
            .anisotropy_enable(false)
            .compare_enable(false)
            .min_lod(0.0)
            .max_lod(0.0)
            .unnormalized_coordinates(false);
        // SAFETY: create_info 只含共享 sampler 契约的穷尽 Vulkan 映射。
        let native = unsafe { device.create_sampler(&create_info, None) }
            .map_err(|error| vk_err("vkCreateSampler RHI", error))?;
        Ok(self.samplers.insert(VulkanRhiSampler { native, desc }))
    }

    fn create_texture(
        &mut self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        desc: TextureDesc,
    ) -> Result<TextureHandle> {
        let resource = create_texture(instance, physical_device, device, desc)?;
        Ok(self.textures.insert(resource))
    }

    fn create_pipeline(
        &mut self,
        device: &ash::Device,
        desc: PipelineDesc,
    ) -> Result<PipelineBinding> {
        let resource = VulkanRhiPipeline::create(device, desc.kind)?;
        Ok(self.pipelines.insert(desc.kind, resource))
    }

    fn update_texture(
        &mut self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        queue: vk::Queue,
        queue_family_index: u32,
        upload: RhiTextureUpload<'_>,
    ) -> Result<()> {
        let texture = self.textures.get(upload.texture())?;
        let validated = upload.validate(texture.desc())?;
        update_texture(
            instance,
            physical_device,
            device,
            queue,
            queue_family_index,
            &mut self.immediate,
            texture,
            validated,
        )
    }

    fn record_texture_copy(
        &mut self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
        copy: TextureCopy,
    ) -> Result<()> {
        self.pass.require_closed()?;
        self.frame.ensure_recording(device, command_buffer)?;
        let source = self.textures.get(copy.source())?;
        let destination = self.textures.get(copy.destination())?;
        let bounds = copy.validate_transfer(source.desc(), destination.desc())?;
        record_texture_copy(device, command_buffer, source, destination, bounds);
        Ok(())
    }

    // 把重叠安全的纹理区域移动录入当前帧，并延迟回收命令引用的临时资源。
    fn record_texture_move(
        &mut self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
        movement: TextureMove,
    ) -> Result<()> {
        self.pass.require_closed()?;
        self.frame.ensure_recording(device, command_buffer)?;
        let source_desc = self.textures.get(movement.source())?.desc();
        let destination_desc = self.textures.get(movement.destination())?.desc();
        movement.validate_transfer(source_desc, destination_desc)?;
        if movement.source() != movement.destination() {
            return self.record_texture_copy(device, command_buffer, movement.into_copy());
        }
        let scratch = self.create_texture(
            instance,
            physical_device,
            device,
            TextureDesc::new(movement.transfer().extent(), source_desc.format()),
        )?;
        // Vulkan 命令缓冲只保存原生句柄引用，临时纹理必须存活到对应 fence 完成。
        self.texture_move_scratch_after_submit.push(scratch);
        let (to_scratch, from_scratch) = movement.through_scratch(scratch);
        self.record_texture_copy(device, command_buffer, to_scratch)?;
        self.record_texture_copy(device, command_buffer, from_scratch)
    }

    // 命令缓冲已重置且覆盖提交已完成后，检查式回收 TextureMove 临时纹理。
    fn reclaim_texture_move_scratch(&mut self, device: &ash::Device) -> Result<()> {
        while let Some(scratch) = self.texture_move_scratch_after_submit.pop() {
            if let Err(error) = self.destroy_texture(device, scratch) {
                // 保留失败句柄供下一次安全边界重试或最终 shutdown 回收。
                self.texture_move_scratch_after_submit.push(scratch);
                return Err(error);
            }
        }
        Ok(())
    }

    // 判断已结束或已提交的命令是否仍拥有待回收 TextureMove 资源。
    pub(super) fn has_texture_move_scratch(&self) -> bool {
        !self.texture_move_scratch_after_submit.is_empty()
    }

    fn preflight_draw_resources(&self, packet: DrawPacket) -> Result<()> {
        self.pipelines.get(packet.pipeline())?;
        self.buffers.validate_draw(packet)?;
        if let Some(binding) = packet.sampling().sampled_texture() {
            let texture = self.textures.get(binding.texture())?;
            let sampler = self.samplers.get(binding.sampler())?;
            binding.validate_resources(texture.desc().format(), sampler.desc)?;
        }
        Ok(())
    }

    pub(super) fn is_frame_prepared(&self) -> bool {
        self.frame.is_prepared()
    }

    pub(super) fn is_frame_recording(&self) -> bool {
        self.frame.is_recording()
    }

    pub(super) fn prepare_frame(
        &mut self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
    ) -> Result<()> {
        self.pass.require_closed()?;
        self.frame.prepare(device, command_buffer)?;
        self.reclaim_texture_move_scratch(device)
    }

    // 复用纹理传输已经拥有的串行即时命令，供同一 Adapter 的 Surface 回读使用。
    pub(super) fn execute_immediate<F>(
        &mut self,
        device: &ash::Device,
        queue: vk::Queue,
        queue_family_index: u32,
        record: F,
    ) -> Result<()>
    where
        F: FnOnce(vk::CommandBuffer),
    {
        // 即时传输不能穿插在仍打开的共享 render pass 中。
        self.pass.require_closed()?;
        // 唯一 immediate owner 负责命令池复用、提交和 GPU 完成等待。
        self.immediate
            .execute(device, queue, queue_family_index, record)
    }

    pub(super) fn begin_render_pass(
        &mut self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
        target: RenderTargetHandle,
        native: VulkanRhiTarget,
        load: LoadAction,
    ) -> Result<()> {
        self.pass.begin(target, native.extent, load)?;
        if let Err(error) = self
            .frame
            .begin_render_pass(device, command_buffer, native, load)
        {
            self.pass.reset();
            return Err(error);
        }
        if let VulkanTargetOwner::Texture(texture) = native.owner {
            self.textures
                .get(texture)?
                .set_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        }
        Ok(())
    }

    pub(super) fn texture_target(&self, texture: TextureHandle) -> Result<VulkanRhiTarget> {
        let resource = self.textures.get(texture)?;
        let desc = resource.desc();
        Ok(VulkanRhiTarget {
            owner: VulkanTargetOwner::Texture(texture),
            image: resource.image(),
            view: resource.view(),
            format: texture_format(desc.format()),
            extent: desc.extent(),
            old_layout: resource.layout(),
            final_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        })
    }

    pub(super) fn clear_rect(
        &mut self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
        color: RhiColor,
        scissor: RhiScissor,
    ) -> Result<()> {
        self.pass.validate_clear(color, scissor)?;
        self.frame
            .clear_rect(device, command_buffer, color, scissor)
    }

    pub(super) fn draw(
        &mut self,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
        packet: DrawPacket,
    ) -> Result<()> {
        self.pass.require_open()?;
        self.preflight_draw_resources(packet)?;
        if !packet.has_valid_sampling() || !packet.has_valid_raster() || !packet.has_valid_range() {
            return Err(rhi_invalid("Vulkan RHI DrawPacket is invalid"));
        }
        self.pass.validate_draw_raster(packet.raster())?;
        if let Some(binding) = packet.sampling().sampled_texture() {
            self.pass.validate_sampled_texture(binding.texture())?;
        }

        let buffers = packet.buffers();
        let range = packet.range();
        let (vertex_upload, uniform_upload, uniform_size, index_binding) = {
            let buffer_table = &self.buffers;
            let frame = &mut self.frame;
            let vertex = buffer_table.get(buffers.vertex())?;
            let uniform = buffer_table.get(buffers.uniform())?;
            let vertex_upload = frame.upload_buffer_snapshot(
                instance,
                physical_device,
                device,
                &vertex.shadow,
                4,
            )?;
            let uniform_upload = frame.upload_buffer_snapshot(
                instance,
                physical_device,
                device,
                &uniform.shadow,
                self.uniform_alignment,
            )?;
            let index_binding = if let Some(binding) = range.index_binding() {
                let index = buffer_table.get(binding.buffer())?;
                let upload = frame.upload_buffer_snapshot(
                    instance,
                    physical_device,
                    device,
                    &index.shadow,
                    4,
                )?;
                Some((upload, index_type(binding.format())))
            } else {
                None
            };
            (
                vertex_upload,
                uniform_upload,
                uniform.desc.size_bytes() as u64,
                index_binding,
            )
        };
        let sampled_native = if let Some(binding) = packet.sampling().sampled_texture() {
            let texture = self.textures.get(binding.texture())?;
            let sampler = self.samplers.get(binding.sampler())?;
            Some((texture.view(), sampler.native))
        } else {
            None
        };

        let (format, render_pass) = self.frame.pipeline_target()?;
        let (pipeline_native, pipeline_layout, descriptor_layout) = {
            let pipeline = self.pipelines.get_mut(packet.pipeline())?;
            let native = pipeline.materialize(device, format, render_pass)?;
            (native, pipeline.layout, pipeline.descriptor_set_layout)
        };
        let descriptor_set = self
            .frame
            .allocate_descriptor_set(device, descriptor_layout)?;
        let buffer_info = [vk::DescriptorBufferInfo::default()
            .buffer(uniform_upload.buffer)
            .offset(uniform_upload.offset)
            .range(uniform_size)];
        let image_info = sampled_native.map(|(view, sampler)| {
            [vk::DescriptorImageInfo::default()
                .sampler(sampler)
                .image_view(view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)]
        });
        let mut writes = Vec::with_capacity(2);
        writes.push(
            vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(&buffer_info),
        );
        if let Some(image_info) = image_info.as_ref() {
            writes.push(
                vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_set)
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(image_info),
            );
        }
        // SAFETY: descriptor 资源、pipeline 与 buffer 均由当前 device 创建并保持存活。
        unsafe {
            device.update_descriptor_sets(&writes, &[]);
            device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_native,
            );
            device.cmd_bind_descriptor_sets(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline_layout,
                0,
                std::slice::from_ref(&descriptor_set),
                &[],
            );
            device.cmd_bind_vertex_buffers(
                command_buffer,
                0,
                std::slice::from_ref(&vertex_upload.buffer),
                std::slice::from_ref(&vertex_upload.offset),
            );
        }
        let extent = self.pass.extent()?;
        self.frame
            .apply_raster(device, command_buffer, packet.raster(), extent);
        // SAFETY: DrawRange、Buffer 容量与索引格式已由共享资源表验证。
        unsafe {
            if let Some((index, index_type)) = index_binding {
                device.cmd_bind_index_buffer(
                    command_buffer,
                    index.buffer,
                    index.offset,
                    index_type,
                );
                device.cmd_draw_indexed(
                    command_buffer,
                    range.index_count(),
                    1,
                    range.first_index(),
                    0,
                    0,
                );
            } else {
                device.cmd_draw(
                    command_buffer,
                    range.vertex_count(),
                    1,
                    range.first_vertex(),
                    0,
                );
            }
        }
        Ok(())
    }

    pub(super) fn end_render_pass(
        &mut self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
    ) -> Result<VulkanCompletedTarget> {
        self.pass.require_open()?;
        let completed = self.frame.end_render_pass(device, command_buffer)?;
        self.pass.end()?;
        if let VulkanTargetOwner::Texture(texture) = completed.owner {
            self.textures.get(texture)?.set_layout(completed.layout);
        }
        Ok(completed)
    }

    pub(super) fn finish_recording(
        &mut self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
    ) -> Result<()> {
        self.pass.require_closed()?;
        self.frame.finish_recording(device, command_buffer)
    }

    pub(super) fn issue_submission(&mut self) -> Result<SubmissionHandle> {
        let submission = self.submissions.issue()?;
        self.frame.mark_submitted();
        Ok(submission)
    }

    pub(super) fn validate_present(
        &self,
        transaction: RhiPresentTransaction,
        current_token: SurfaceToken,
        coherency: crate::core::PresentCoherency,
    ) -> Result<ValidatedRhiPresent> {
        transaction.validate(current_token, coherency, &self.submissions)
    }

    pub(super) fn validate_latest_submission(
        &self,
        submission: SubmissionHandle,
    ) -> Result<SubmissionHandle> {
        self.submissions.validate(submission)?;
        Ok(submission)
    }

    fn destroy_buffer(&mut self, device: &ash::Device, handle: BufferHandle) -> Result<()> {
        let resource = self.buffers.take(handle)?;
        // SAFETY: 资源表刚移交唯一对象，调用方保证没有在途提交引用。
        unsafe {
            device.destroy_buffer(resource.native, None);
            device.free_memory(resource.memory, None);
        }
        Ok(())
    }

    fn destroy_sampler(&mut self, device: &ash::Device, handle: SamplerHandle) -> Result<()> {
        let resource = self.samplers.take(handle)?;
        // SAFETY: 资源表刚移交唯一对象，Sampler 没有独立内存或子资源。
        unsafe { device.destroy_sampler(resource.native, None) };
        Ok(())
    }

    fn destroy_texture(&mut self, device: &ash::Device, handle: TextureHandle) -> Result<()> {
        self.pass.validate_texture_destroy(handle)?;
        let resource = self.textures.take(handle)?;
        destroy_texture(device, resource);
        Ok(())
    }

    fn destroy_pipeline(&mut self, device: &ash::Device, binding: PipelineBinding) -> Result<()> {
        let resource = self.pipelines.take(binding)?;
        resource.destroy(device);
        Ok(())
    }

    pub(super) fn shutdown(&mut self, device: &ash::Device) {
        // 即时命令池必须在其记录引用的 Image 资源前完成队列排空；调用方已等待 Device idle。
        self.immediate.shutdown(device);
        // 单帧 descriptor、framebuffer 与 render pass 必须先于资源布局整体释放。
        self.frame.shutdown(device);
        // PipelineLayout 引用 DescriptorSetLayout，必须先按资源创建逆序整体回收。
        for pipeline in self.pipelines.drain_reverse() {
            pipeline.destroy(device);
        }
        // Sampler 与 Buffer 没有父子关系；均按各自创建逆序回收。
        for sampler in self.samplers.drain_reverse() {
            // SAFETY: device 已 idle 或 lost，资源表移交的对象不会再被引用。
            unsafe { device.destroy_sampler(sampler.native, None) };
        }
        for buffer in self.buffers.drain_reverse() {
            // SAFETY: device 已 idle 或 lost，先删 Buffer 再释放其绑定内存。
            unsafe {
                device.destroy_buffer(buffer.native, None);
                device.free_memory(buffer.memory, None);
            }
        }
        // Device 已空闲，资源表整体回收会覆盖尚未进入下一 prepare 的临时纹理。
        self.texture_move_scratch_after_submit.clear();
        for texture in self.textures.drain_reverse() {
            destroy_texture(device, texture);
        }
        self.pass.reset();
        self.submissions.invalidate();
    }
}

impl GraphicsDevice for VulkanContext {
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
        // Vulkan 已实现通用 Renderer 的完整 Device 基线、局部清理与安全区域移动。
        let mut capabilities = GraphicsDeviceCapabilities::full_gpu_baseline();
        capabilities.clear_rect = true;
        capabilities.texture_region_move = true;
        capabilities
    }

    fn create_buffer(&mut self, desc: BufferDesc) -> Result<BufferHandle> {
        let owner = self.active_device()?;
        let instance = self
            .runtime
            .as_ref()
            .ok_or_else(|| vulkan_rhi_unavailable("create_buffer after shutdown"))?
            .instance()
            .clone();
        let result =
            self.rhi_device
                .create_buffer(&instance, self.physical_device, &self.device, desc);
        owner.observe(result)
    }

    fn update_buffer(&mut self, upload: RhiBufferUpload<'_>) -> Result<()> {
        let owner = self.active_device()?;
        let result = self.rhi_device.update_buffer(&self.device, upload);
        owner.observe(result)
    }

    fn preflight_buffer_upload(&self, upload: RhiBufferUploadPreflight) -> Result<()> {
        self.rhi_device.buffers.validate_upload(upload)
    }

    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureHandle> {
        let owner = self.active_device()?;
        let instance = self
            .runtime
            .as_ref()
            .ok_or_else(|| vulkan_rhi_unavailable("create_texture after shutdown"))?
            .instance()
            .clone();
        let result =
            self.rhi_device
                .create_texture(&instance, self.physical_device, &self.device, desc);
        owner.observe(result)
    }

    fn resolve_render_target(&self, texture: TextureHandle) -> Result<RenderTargetHandle> {
        self.rhi_device.textures.resolve_render_target(texture)
    }

    fn preflight_texture_copy(&self, copy: TextureCopy) -> Result<()> {
        self.rhi_device.textures.validate_copy(copy)
    }

    fn preflight_texture_move(&self, movement: TextureMove) -> Result<()> {
        self.rhi_device.textures.validate_move(movement)
    }

    fn preflight_draw_resources(&self, packet: DrawPacket) -> Result<()> {
        self.rhi_device.preflight_draw_resources(packet)
    }

    fn update_texture(&mut self, upload: RhiTextureUpload<'_>) -> Result<()> {
        let owner = self.active_device()?;
        if self.rhi_device.is_frame_recording() {
            return Err(rhi_invalid(
                "Vulkan RHI texture upload cannot interrupt frame recording",
            ));
        }
        let instance = self
            .runtime
            .as_ref()
            .ok_or_else(|| vulkan_rhi_unavailable("update_texture after shutdown"))?
            .instance()
            .clone();
        let result = owner.with_queue("vkQueueSubmit RHI texture upload", |queue| {
            self.rhi_device.update_texture(
                &instance,
                self.physical_device,
                &self.device,
                queue,
                self.adapter_info.queue_family_index,
                upload,
            )
        })?;
        owner.observe(result)
    }

    fn create_sampler(&mut self, desc: SamplerDesc) -> Result<SamplerHandle> {
        let owner = self.active_device()?;
        let result = self.rhi_device.create_sampler(&self.device, desc);
        owner.observe(result)
    }

    fn create_pipeline(&mut self, desc: PipelineDesc) -> Result<PipelineBinding> {
        let owner = self.active_device()?;
        let result = self.rhi_device.create_pipeline(&self.device, desc);
        owner.observe(result)
    }

    fn destroy_buffer(&mut self, buffer: BufferHandle) -> Result<()> {
        let owner = self.active_device()?;
        let result = self
            .ensure_rhi_frame_prepared()
            .and_then(|()| self.rhi_device.destroy_buffer(&self.device, buffer));
        owner.observe(result)
    }

    fn destroy_sampler(&mut self, sampler: SamplerHandle) -> Result<()> {
        let owner = self.active_device()?;
        let result = self
            .ensure_rhi_frame_prepared()
            .and_then(|()| self.rhi_device.destroy_sampler(&self.device, sampler));
        owner.observe(result)
    }

    fn destroy_texture(&mut self, texture: TextureHandle) -> Result<()> {
        let owner = self.active_device()?;
        let result = self
            .ensure_rhi_frame_prepared()
            .and_then(|()| self.rhi_device.destroy_texture(&self.device, texture));
        owner.observe(result)
    }

    fn destroy_pipeline(&mut self, pipeline: PipelineBinding) -> Result<()> {
        let owner = self.active_device()?;
        let result = self
            .ensure_rhi_frame_prepared()
            .and_then(|()| self.rhi_device.destroy_pipeline(&self.device, pipeline));
        owner.observe(result)
    }

    fn begin_render_pass(&mut self, target: RenderTargetHandle, load: LoadAction) -> Result<()> {
        let owner = self.active_device()?;
        let result = (|| {
            self.ensure_rhi_commands_ready()?;
            let native = if target.is_surface() {
                self.acquired_surface_target()?
            } else {
                let texture = target
                    .texture()
                    .ok_or_else(|| rhi_invalid("Vulkan RHI render target identity is invalid"))?;
                self.rhi_device.texture_target(texture)?
            };
            self.rhi_device.begin_render_pass(
                &self.device,
                self.command_buffer,
                target,
                native,
                load,
            )?;
            if let VulkanTargetOwner::Surface(image_slot) = native.owner {
                self.image_layouts[image_slot] = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
            }
            Ok(())
        })();
        owner.observe(result)
    }

    fn clear_rect(&mut self, color: RhiColor, scissor: RhiScissor) -> Result<()> {
        let owner = self.active_device()?;
        let result = self
            .rhi_device
            .clear_rect(&self.device, self.command_buffer, color, scissor);
        owner.observe(result)
    }

    fn draw(&mut self, packet: DrawPacket) -> Result<()> {
        let owner = self.active_device()?;
        let instance = self
            .runtime
            .as_ref()
            .ok_or_else(|| vulkan_rhi_unavailable("draw after shutdown"))?
            .instance()
            .clone();
        let result = self.rhi_device.draw(
            &instance,
            self.physical_device,
            &self.device,
            self.command_buffer,
            packet,
        );
        owner.observe(result)
    }

    fn copy_texture(&mut self, copy: TextureCopy) -> Result<()> {
        let owner = self.active_device()?;
        let result = self.ensure_rhi_commands_ready().and_then(|()| {
            self.rhi_device
                .record_texture_copy(&self.device, self.command_buffer, copy)
        });
        owner.observe(result)
    }

    fn move_texture_region(&mut self, movement: TextureMove) -> Result<()> {
        let owner = self.active_device()?;
        let instance = self
            .runtime
            .as_ref()
            .ok_or_else(|| vulkan_rhi_unavailable("move_texture_region after shutdown"))?
            .instance()
            .clone();
        let result = self.ensure_rhi_commands_ready().and_then(|()| {
            self.rhi_device.record_texture_move(
                &instance,
                self.physical_device,
                &self.device,
                self.command_buffer,
                movement,
            )
        });
        owner.observe(result)
    }

    fn end_render_pass(&mut self) -> Result<()> {
        let owner = self.active_device()?;
        let result = self
            .rhi_device
            .end_render_pass(&self.device, self.command_buffer)
            .map(|completed| {
                if let VulkanTargetOwner::Surface(image_slot) = completed.owner {
                    self.image_layouts[image_slot] = completed.layout;
                }
            });
        owner.observe(result)
    }

    fn submit(&mut self) -> Result<SubmissionHandle> {
        let owner = self.active_device()?;
        let result = self.submit_rhi_frame(&owner);
        owner.observe(result)
    }

    fn maintain(&mut self) -> Result<()> {
        self.active_device()?.ensure_healthy()
    }
}

fn vulkan_rhi_unavailable(operation: &'static str) -> Error {
    Error::new(
        Errc::GraphicsDeviceLost,
        format!("Vulkan GPU-native RHI cannot {operation}"),
    )
}

fn rhi_invalid(message: &'static str) -> Error {
    Error::new(Errc::InvalidArgument, message)
}

fn buffer_shadow_oom() -> Error {
    Error::new(
        Errc::GraphicsOutOfMemory,
        "Vulkan RHI buffer shadow allocation failed",
    )
}

// 把真实 GPU 测试入口限制在显式 feature 内，并保持实现为当前模块私有子组件。
#[cfg(feature = "vulkan-parity-test")]
pub(super) fn run_gpu_parity_test() {
    gpu_parity_tests::run_gpu_parity_test();
}

// 真实 UI/Drawing 生产链只复用 Adapter 的机械回读，不在此定义期望或容差。
#[cfg(feature = "vulkan-parity-test")]
pub(super) fn readback_production_texture(
    context: &mut VulkanContext,
    texture: TextureHandle,
) -> Vec<u8> {
    gpu_parity_tests::readback_production_texture(context, texture)
}

// 把真实 GPU parity harness 实现统一存放在根 tests/support 目录。
#[cfg(feature = "vulkan-parity-test")]
#[path = "../../../../../../../tests/support/native/gpu_parity/vulkan_rhi_device.rs"]
mod gpu_parity_tests;
