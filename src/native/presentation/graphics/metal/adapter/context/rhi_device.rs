//! Metal thin RHI 的资源、传输与提交 owner。

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLBlitCommandEncoder, MTLBuffer, MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandEncoder,
    MTLCommandQueue, MTLDevice, MTLLibrary, MTLOrigin, MTLRenderCommandEncoder,
    MTLRenderPipelineState, MTLResourceOptions, MTLSamplerAddressMode, MTLSamplerDescriptor,
    MTLSamplerMinMagFilter, MTLSamplerMipFilter, MTLSamplerState, MTLSize, MTLStorageMode,
    MTLTexture, MTLTextureDescriptor, MTLTextureUsage,
};

use crate::core::{Errc, Error, Result};
use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, DrawPacket, GraphicsDevice, GraphicsDeviceCapabilities, LoadAction,
    PipelineBinding, PipelineDesc, RenderTargetHandle, RhiBufferResource, RhiBufferResourceTable,
    RhiBufferUpload, RhiBufferUploadPreflight, RhiColor, RhiPassState, RhiPipelineResourceTable,
    RhiResourceTable, RhiScissor, RhiSubmissionSequence, RhiTextureResource,
    RhiTextureResourceTable, RhiTextureTransferBounds, RhiTextureUpload, SamplerAddressMode,
    SamplerDesc, SamplerFilter, SamplerHandle, SamplerMipMode, SubmissionHandle, TextureCopy,
    TextureDesc, TextureFormat, TextureHandle, TextureMove, UIX_COLOR_CONTRACT,
};

use super::super::pipeline::{self, MetalPipelineResource};
use super::{MetalContext, metal_command_buffer_error};

pub(super) struct MetalRhiBuffer {
    pub(super) native: Retained<ProtocolObject<dyn MTLBuffer>>,
    desc: BufferDesc,
    shadow: Vec<u8>,
}

impl RhiBufferResource for MetalRhiBuffer {
    fn desc(&self) -> BufferDesc {
        self.desc
    }
}

pub(super) struct MetalRhiTexture {
    pub(super) native: Retained<ProtocolObject<dyn MTLTexture>>,
    pub(super) desc: TextureDesc,
}

impl RhiTextureResource for MetalRhiTexture {
    fn desc(&self) -> TextureDesc {
        self.desc
    }
}

pub(super) struct MetalRhiSampler {
    pub(super) native: Retained<ProtocolObject<dyn MTLSamplerState>>,
    pub(super) desc: SamplerDesc,
}

pub(super) struct MetalRhiDevice {
    pub(super) buffers: RhiBufferResourceTable<MetalRhiBuffer>,
    pub(super) textures: RhiTextureResourceTable<MetalRhiTexture>,
    pub(super) samplers: RhiResourceTable<SamplerHandle, MetalRhiSampler>,
    pub(super) pipelines: RhiPipelineResourceTable<MetalPipelineResource>,
    pub(super) pass: RhiPassState,
    pub(super) command_buffer: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    pub(super) render_encoder: Option<Retained<ProtocolObject<dyn MTLRenderCommandEncoder>>>,
    pub(super) active_target: Option<RenderTargetHandle>,
    pub(super) active_format: Option<TextureFormat>,
    pub(super) pending_buffers: Vec<Retained<ProtocolObject<dyn MTLBuffer>>>,
    pub(super) pending_textures: Vec<Retained<ProtocolObject<dyn MTLTexture>>>,
    pub(super) submission_sequence: RhiSubmissionSequence,
    pub(super) library: Retained<ProtocolObject<dyn MTLLibrary>>,
    pub(super) clear_bgra: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub(super) clear_rgba: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
}

impl MetalRhiDevice {
    pub(super) fn new(
        device: &ProtocolObject<dyn MTLDevice>,
        library: Retained<ProtocolObject<dyn MTLLibrary>>,
    ) -> Result<Self> {
        let clear_bgra =
            pipeline::create_clear_pipeline(device, &library, TextureFormat::Bgra8Unorm)?;
        let clear_rgba =
            pipeline::create_clear_pipeline(device, &library, TextureFormat::Rgba8Unorm)?;
        Ok(Self {
            buffers: RhiBufferResourceTable::new(),
            textures: RhiTextureResourceTable::new(),
            samplers: RhiResourceTable::new(),
            pipelines: RhiPipelineResourceTable::new(),
            pass: RhiPassState::new(),
            command_buffer: None,
            render_encoder: None,
            active_target: None,
            active_format: None,
            pending_buffers: Vec::new(),
            pending_textures: Vec::new(),
            submission_sequence: RhiSubmissionSequence::new(),
            library,
            clear_bgra,
            clear_rgba,
        })
    }

    pub(super) fn has_pending_commands(&self) -> bool {
        self.pass.is_open() || self.command_buffer.is_some()
    }

    // 丢弃尚未提交的单帧编码状态，让 Surface 失败回滚后可以重新 acquire。
    pub(super) fn discard_pending_frame(&mut self) {
        if let Some(encoder) = self.render_encoder.take() {
            encoder.endEncoding();
        }
        self.command_buffer = None;
        self.pending_buffers.clear();
        self.pending_textures.clear();
        self.pass.reset();
        self.active_target = None;
        self.active_format = None;
    }

    pub(super) fn ensure_command_buffer(
        &mut self,
        queue: &ProtocolObject<dyn MTLCommandQueue>,
    ) -> Result<&ProtocolObject<dyn MTLCommandBuffer>> {
        if self.command_buffer.is_none() {
            self.command_buffer = Some(queue.commandBuffer().ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "Metal command queue failed to create a command buffer",
                )
            })?);
        }
        Ok(self.command_buffer.as_deref().expect("just initialized"))
    }

    fn capabilities(&self) -> GraphicsDeviceCapabilities {
        GraphicsDeviceCapabilities {
            color_contract: UIX_COLOR_CONTRACT,
            dynamic_buffers: true,
            texture_upload: true,
            texture_copy: true,
            texture_region_move: true,
            clear_rect: true,
            sampled_textures: true,
            render_to_texture: true,
            scissor: true,
            premultiplied_alpha_blend: true,
            additive_blend: true,
        }
    }

    fn create_buffer(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        desc: BufferDesc,
    ) -> Result<BufferHandle> {
        desc.validate()?;
        let shadow = vec![0; desc.size_bytes()];
        let native = create_shared_buffer(device, &shadow)?;
        Ok(self.buffers.insert(MetalRhiBuffer {
            native,
            desc,
            shadow,
        }))
    }

    fn update_buffer(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        upload: RhiBufferUpload<'_>,
    ) -> Result<()> {
        let resource = self.buffers.get(upload.buffer())?;
        let validated = upload.validate(resource.desc)?;
        let mut shadow = resource.shadow.clone();
        shadow[..validated.data().len()].copy_from_slice(validated.data());
        let native = create_shared_buffer(device, &shadow)?;
        let resource = self.buffers.get_mut(upload.buffer())?;
        resource.native = native;
        resource.shadow = shadow;
        Ok(())
    }

    fn create_texture(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        desc: TextureDesc,
    ) -> Result<TextureHandle> {
        let native = create_native_texture(device, desc)?;
        Ok(self.textures.insert(MetalRhiTexture { native, desc }))
    }

    fn update_texture(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        queue: &ProtocolObject<dyn MTLCommandQueue>,
        upload: RhiTextureUpload<'_>,
    ) -> Result<()> {
        self.pass.require_closed()?;
        let texture = self.textures.get(upload.texture())?;
        let validated = upload.validate(texture.desc)?;
        let target = texture.native.clone();
        let staging = create_shared_buffer(device, validated.data())?;
        let bounds = validated.bounds();
        let command = self.ensure_command_buffer(queue)?;
        let encoder = command.blitCommandEncoder().ok_or_else(|| {
            Error::new(Errc::PlatformError, "Metal failed to create a blit encoder")
        })?;
        let region = bounds.region();
        // SAFETY: 共享上传门禁已证明 buffer 长度、行跨度和目标区域完整有效。
        // 二维纹理按 Metal 契约把 sourceBytesPerImage 固定为零。
        unsafe {
            encoder.copyFromBuffer_sourceOffset_sourceBytesPerRow_sourceBytesPerImage_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                &staging,
                0,
                validated.row_pitch() as usize,
                0,
                metal_size(region.extent()),
                &target,
                0,
                0,
                metal_origin(region.origin().x(), region.origin().y()),
            );
        }
        encoder.endEncoding();
        self.pending_buffers.push(staging);
        Ok(())
    }

    fn create_sampler(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        desc: SamplerDesc,
    ) -> Result<SamplerHandle> {
        if desc.address_mode() != SamplerAddressMode::ClampToEdge
            || desc.mip_mode() != SamplerMipMode::SingleLevel
        {
            return Err(Error::new(
                Errc::InvalidArgument,
                "Metal RHI sampler description is unsupported",
            ));
        }
        let descriptor = MTLSamplerDescriptor::new();
        let filter = match desc.filter() {
            SamplerFilter::Nearest => MTLSamplerMinMagFilter::Nearest,
            SamplerFilter::Linear => MTLSamplerMinMagFilter::Linear,
        };
        descriptor.setMinFilter(filter);
        descriptor.setMagFilter(filter);
        descriptor.setMipFilter(MTLSamplerMipFilter::NotMipmapped);
        descriptor.setSAddressMode(MTLSamplerAddressMode::ClampToEdge);
        descriptor.setTAddressMode(MTLSamplerAddressMode::ClampToEdge);
        let native = device
            .newSamplerStateWithDescriptor(&descriptor)
            .ok_or_else(|| Error::new(Errc::PlatformError, "Metal failed to create sampler"))?;
        Ok(self.samplers.insert(MetalRhiSampler { native, desc }))
    }

    fn create_pipeline(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        desc: PipelineDesc,
    ) -> Result<PipelineBinding> {
        let native = pipeline::create_pipeline(device, &self.library, desc)?;
        Ok(self.pipelines.insert(desc.kind, native))
    }

    pub(super) fn preflight_draw_resources(&self, packet: DrawPacket) -> Result<()> {
        self.buffers.validate_draw(packet)?;
        self.pipelines.get(packet.pipeline())?;
        if let Some(binding) = packet.sampling().sampled_texture() {
            self.pass.validate_sampled_texture(binding.texture())?;
            let texture = self.textures.get(binding.texture())?;
            let sampler = self.samplers.get(binding.sampler())?;
            binding.validate_resources(texture.desc.format(), sampler.desc)?;
        }
        self.pass.validate_draw_raster(packet.raster())
    }

    fn copy_texture(
        &mut self,
        queue: &ProtocolObject<dyn MTLCommandQueue>,
        copy: TextureCopy,
    ) -> Result<()> {
        self.pass.require_closed()?;
        let source = self.textures.get(copy.source())?;
        let destination = self.textures.get(copy.destination())?;
        let bounds = copy.validate_transfer(source.desc, destination.desc)?;
        let source_native = source.native.clone();
        let destination_native = destination.native.clone();
        self.encode_copy(queue, &source_native, &destination_native, bounds)
    }

    fn move_texture(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        queue: &ProtocolObject<dyn MTLCommandQueue>,
        movement: TextureMove,
    ) -> Result<()> {
        self.pass.require_closed()?;
        if movement.source() != movement.destination() {
            return self.copy_texture(queue, movement.into_copy());
        }
        let source = self.textures.get(movement.source())?;
        let bounds = movement.validate_transfer(source.desc, source.desc)?;
        let source_native = source.native.clone();
        let scratch_desc =
            TextureDesc::new(bounds.source().region().extent(), source.desc.format());
        let scratch = create_native_texture(device, scratch_desc)?;
        use crate::platform::presentation::rhi::{RhiTextureOrigin, RhiTextureTransfer};
        let extent = bounds.source().region().extent();
        let to_scratch = RhiTextureTransfer::new(
            bounds.source().region().origin(),
            RhiTextureOrigin::new(0, 0),
            extent,
        )
        .validate(source.desc, scratch_desc)?;
        let from_scratch = RhiTextureTransfer::new(
            RhiTextureOrigin::new(0, 0),
            bounds.destination().region().origin(),
            extent,
        )
        .validate(scratch_desc, source.desc)?;
        self.encode_copy(queue, &source_native, &scratch, to_scratch)?;
        self.encode_copy(queue, &scratch, &source_native, from_scratch)?;
        self.pending_textures.push(scratch);
        Ok(())
    }

    fn encode_copy(
        &mut self,
        queue: &ProtocolObject<dyn MTLCommandQueue>,
        source: &ProtocolObject<dyn MTLTexture>,
        destination: &ProtocolObject<dyn MTLTexture>,
        bounds: RhiTextureTransferBounds,
    ) -> Result<()> {
        let command = self.ensure_command_buffer(queue)?;
        let encoder = command.blitCommandEncoder().ok_or_else(|| {
            Error::new(Errc::PlatformError, "Metal failed to create a blit encoder")
        })?;
        let source_region = bounds.source().region();
        let destination_region = bounds.destination().region();
        // SAFETY: 共享 copy 门禁已证明两端格式、范围和唯一尺寸完全匹配。
        unsafe {
            encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
                source,
                0,
                0,
                metal_origin(source_region.origin().x(), source_region.origin().y()),
                metal_size(source_region.extent()),
                destination,
                0,
                0,
                metal_origin(destination_region.origin().x(), destination_region.origin().y()),
            );
        }
        encoder.endEncoding();
        Ok(())
    }

    fn submit(&mut self) -> Result<SubmissionHandle> {
        self.pass.require_closed()?;
        let command = self.command_buffer.take().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "Metal RHI submit requires encoded commands",
            )
        })?;
        command.commit();
        command.waitUntilCompleted();
        self.pending_buffers.clear();
        self.pending_textures.clear();
        if command.status() != MTLCommandBufferStatus::Completed {
            let error = command.error();
            return Err(metal_command_buffer_error(
                "command buffer",
                error.as_deref(),
                Errc::GraphicsDeviceLost,
            ));
        }
        self.submission_sequence.issue()
    }

    pub(super) fn shutdown(&mut self) -> Result<()> {
        self.discard_pending_frame();
        self.submission_sequence.invalidate();
        for _ in self.pipelines.drain_reverse() {}
        for _ in self.samplers.drain_reverse() {}
        for _ in self.textures.drain_reverse() {}
        for _ in self.buffers.drain_reverse() {}
        Ok(())
    }
}

impl GraphicsDevice for MetalContext {
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
        self.rhi_device.capabilities()
    }

    fn create_buffer(&mut self, desc: BufferDesc) -> Result<BufferHandle> {
        self.ensure_healthy()?;
        self.rhi_device.create_buffer(&self.device, desc)
    }

    fn update_buffer(&mut self, upload: RhiBufferUpload<'_>) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.update_buffer(&self.device, upload)
    }

    fn preflight_buffer_upload(&self, upload: RhiBufferUploadPreflight) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.buffers.validate_upload(upload)
    }

    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureHandle> {
        self.ensure_healthy()?;
        self.rhi_device.create_texture(&self.device, desc)
    }

    fn resolve_render_target(&self, texture: TextureHandle) -> Result<RenderTargetHandle> {
        self.ensure_healthy()?;
        self.rhi_device.textures.resolve_render_target(texture)
    }

    fn preflight_texture_copy(&self, copy: TextureCopy) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.textures.validate_copy(copy)
    }

    fn preflight_texture_move(&self, movement: TextureMove) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.textures.validate_move(movement)
    }

    fn preflight_draw_resources(&self, packet: DrawPacket) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.preflight_draw_resources(packet)
    }

    fn update_texture(&mut self, upload: RhiTextureUpload<'_>) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device
            .update_texture(&self.device, &self.queue, upload)
    }

    fn create_sampler(&mut self, desc: SamplerDesc) -> Result<SamplerHandle> {
        self.ensure_healthy()?;
        self.rhi_device.create_sampler(&self.device, desc)
    }

    fn create_pipeline(&mut self, desc: PipelineDesc) -> Result<PipelineBinding> {
        self.ensure_healthy()?;
        self.rhi_device.create_pipeline(&self.device, desc)
    }

    fn destroy_buffer(&mut self, buffer: BufferHandle) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.pass.require_closed()?;
        drop(self.rhi_device.buffers.take(buffer)?);
        Ok(())
    }

    fn destroy_texture(&mut self, texture: TextureHandle) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.pass.validate_texture_destroy(texture)?;
        drop(self.rhi_device.textures.take(texture)?);
        Ok(())
    }

    fn destroy_sampler(&mut self, sampler: SamplerHandle) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.pass.require_closed()?;
        drop(self.rhi_device.samplers.take(sampler)?);
        Ok(())
    }

    fn destroy_pipeline(&mut self, pipeline: PipelineBinding) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.pass.require_closed()?;
        drop(self.rhi_device.pipelines.take(pipeline)?);
        Ok(())
    }

    fn begin_render_pass(&mut self, target: RenderTargetHandle, load: LoadAction) -> Result<()> {
        self.begin_metal_render_pass(target, load)
    }

    fn clear_rect(&mut self, color: RhiColor, scissor: RhiScissor) -> Result<()> {
        self.clear_metal_rect(color, scissor)
    }

    fn draw(&mut self, packet: DrawPacket) -> Result<()> {
        self.draw_metal(packet)
    }

    fn copy_texture(&mut self, copy: TextureCopy) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.copy_texture(&self.queue, copy)
    }

    fn move_texture_region(&mut self, movement: TextureMove) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device
            .move_texture(&self.device, &self.queue, movement)
    }

    fn end_render_pass(&mut self) -> Result<()> {
        self.end_metal_render_pass()
    }

    fn submit(&mut self) -> Result<SubmissionHandle> {
        self.ensure_healthy()?;
        let result = self.rhi_device.submit();
        if let Err(error) = &result {
            // 提交失败的 surface 帧不可再呈现，释放 drawable 让恢复路径重新 acquire。
            self.acquired_drawable = None;
            self.record_fault(error);
        }
        result
    }

    fn maintain(&mut self) -> Result<()> {
        self.ensure_healthy()
    }
}

fn create_shared_buffer(
    device: &ProtocolObject<dyn MTLDevice>,
    data: &[u8],
) -> Result<Retained<ProtocolObject<dyn MTLBuffer>>> {
    if data.is_empty() {
        return Err(Error::new(
            Errc::InvalidArgument,
            "Metal RHI buffer payload is empty",
        ));
    }
    let buffer = device
        .newBufferWithLength_options(data.len(), MTLResourceOptions::StorageModeShared)
        .ok_or_else(|| Error::new(Errc::PlatformError, "Metal failed to allocate buffer"))?;
    // SAFETY: 新建 shared buffer 至少与 data 等长，两个区域不重叠。
    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr(), buffer.contents().as_ptr().cast(), data.len());
    }
    Ok(buffer)
}

pub(super) fn create_native_texture(
    device: &ProtocolObject<dyn MTLDevice>,
    desc: TextureDesc,
) -> Result<Retained<ProtocolObject<dyn MTLTexture>>> {
    desc.validate()?;
    let descriptor = unsafe {
        MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
            pipeline::texture_pixel_format(desc.format())?,
            desc.extent().width as usize,
            desc.extent().height as usize,
            false,
        )
    };
    descriptor.setStorageMode(MTLStorageMode::Private);
    let mut usage = MTLTextureUsage::ShaderRead;
    if desc.format().supports_render_target() {
        usage |= MTLTextureUsage::RenderTarget;
    }
    descriptor.setUsage(usage);
    device
        .newTextureWithDescriptor(&descriptor)
        .ok_or_else(|| Error::new(Errc::PlatformError, "Metal failed to allocate texture"))
}

pub(super) const fn metal_origin(x: u32, y: u32) -> MTLOrigin {
    MTLOrigin {
        x: x as usize,
        y: y as usize,
        z: 0,
    }
}

pub(super) const fn metal_size(extent: crate::platform::presentation::rhi::RhiExtent) -> MTLSize {
    MTLSize {
        width: extent.width as usize,
        height: extent.height as usize,
        depth: 1,
    }
}
