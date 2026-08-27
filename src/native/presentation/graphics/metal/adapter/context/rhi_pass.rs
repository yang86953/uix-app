//! Metal render pass、draw 与局部清理编码。

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_metal::{
    MTLCommandBuffer, MTLCommandEncoder, MTLIndexType, MTLLoadAction, MTLPrimitiveType,
    MTLRenderCommandEncoder, MTLRenderPassDescriptor, MTLScissorRect, MTLStoreAction, MTLViewport,
};
use objc2_quartz_core::CAMetalDrawable;

use crate::core::{Errc, Error, Result};
use crate::platform::presentation::rhi::{
    DrawPacket, LoadAction, RenderTargetHandle, RhiColor, RhiScissor, TextureFormat,
};

use super::MetalContext;

impl MetalContext {
    pub(super) fn begin_metal_render_pass(
        &mut self,
        target: RenderTargetHandle,
        load: LoadAction,
    ) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.pass.require_closed()?;
        let (texture, format, extent) = match target {
            RenderTargetHandle::Surface => {
                let drawable = self.acquired_drawable.as_ref().ok_or_else(|| {
                    Error::new(
                        Errc::InvalidState,
                        "Metal surface render pass requires an acquired drawable",
                    )
                })?;
                (
                    drawable.texture(),
                    TextureFormat::Bgra8Unorm,
                    self.surface_lifecycle.token().extent,
                )
            }
            RenderTargetHandle::Texture(handle) => {
                let resource = self.rhi_device.textures.get(handle)?;
                (
                    resource.native.clone(),
                    resource.desc.format(),
                    resource.desc.extent(),
                )
            }
        };
        self.rhi_device.pass.begin(target, extent, load)?;
        let descriptor = MTLRenderPassDescriptor::new();
        let attachments = descriptor.colorAttachments();
        // SAFETY: Metal render pass descriptor 固定拥有颜色附件零槽位。
        let attachment = unsafe { attachments.objectAtIndexedSubscript(0) };
        attachment.setTexture(Some(&texture));
        attachment.setStoreAction(MTLStoreAction::Store);
        match load {
            LoadAction::Load => attachment.setLoadAction(MTLLoadAction::Load),
            LoadAction::Clear(color) => {
                attachment.setLoadAction(MTLLoadAction::Clear);
                let [red, green, blue, alpha] = color.components();
                attachment.setClearColor(objc2_metal::MTLClearColor {
                    red: red as f64,
                    green: green as f64,
                    blue: blue as f64,
                    alpha: alpha as f64,
                });
            }
        }
        let command = match self.rhi_device.ensure_command_buffer(&self.queue) {
            Ok(command) => command,
            Err(error) => {
                self.discard_acquired_frame_state();
                return Err(error);
            }
        };
        let encoder = command
            .renderCommandEncoderWithDescriptor(&descriptor)
            .ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "Metal failed to create a render command encoder",
                )
            });
        let encoder = match encoder {
            Ok(encoder) => encoder,
            Err(error) => {
                // 原生 encoder 未建立时回滚共享 pass、命令缓冲与 Surface drawable。
                self.discard_acquired_frame_state();
                return Err(error);
            }
        };
        self.rhi_device.render_encoder = Some(encoder);
        self.rhi_device.active_target = Some(target);
        self.rhi_device.active_format = Some(format);
        self.rhi_device.pending_textures.push(texture);
        Ok(())
    }

    pub(super) fn clear_metal_rect(&mut self, color: RhiColor, scissor: RhiScissor) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.pass.validate_clear(color, scissor)?;
        let format = self.rhi_device.active_format.ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "Metal clear has no active target format",
            )
        })?;
        let pipeline = match format {
            TextureFormat::Bgra8Unorm => self.rhi_device.clear_bgra.clone(),
            TextureFormat::Rgba8Unorm => self.rhi_device.clear_rgba.clone(),
            TextureFormat::R8Unorm => {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    "Metal cannot clear an R8 render target",
                ));
            }
        };
        let encoder = self.rhi_device.render_encoder.as_deref().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "Metal clear has no active render encoder",
            )
        })?;
        let extent = self.rhi_device.pass.extent()?;
        encoder.setRenderPipelineState(&pipeline);
        // 局部清理不继承上一条 draw 的 viewport，始终以完整活动目标解释 scissor。
        encoder.setViewport(MTLViewport {
            originX: 0.0,
            originY: 0.0,
            width: extent.width as f64,
            height: extent.height as f64,
            znear: 0.0,
            zfar: 1.0,
        });
        encoder.setScissorRect(metal_scissor(scissor));
        let components = color.components();
        let pointer = NonNull::new(components.as_ptr().cast_mut().cast::<c_void>())
            .expect("four color components are non-null");
        // SAFETY: MSL clear_fs 读取恰好一个 float4，指针在调用期间有效。
        unsafe {
            encoder.setFragmentBytes_length_atIndex(pointer, size_of::<[f32; 4]>(), 0);
            encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 3);
        }
        Ok(())
    }

    pub(super) fn draw_metal(&mut self, packet: DrawPacket) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.preflight_draw_resources(packet)?;
        let format = self.rhi_device.active_format.ok_or_else(|| {
            Error::new(Errc::InvalidState, "Metal draw has no active target format")
        })?;
        let pipeline = self
            .rhi_device
            .pipelines
            .get(packet.pipeline())?
            .for_format(format)?;
        let buffers = packet.buffers();
        let vertex = self
            .rhi_device
            .buffers
            .get(buffers.vertex())?
            .native
            .clone();
        let uniform = self
            .rhi_device
            .buffers
            .get(buffers.uniform())?
            .native
            .clone();
        let sampled = packet
            .sampling()
            .sampled_texture()
            .map(|binding| {
                Ok::<_, Error>((
                    self.rhi_device
                        .textures
                        .get(binding.texture())?
                        .native
                        .clone(),
                    self.rhi_device
                        .samplers
                        .get(binding.sampler())?
                        .native
                        .clone(),
                ))
            })
            .transpose()?;
        let index = packet
            .range()
            .index_binding()
            .map(|binding| {
                self.rhi_device
                    .buffers
                    .get(binding.buffer())
                    .map(|entry| entry.native.clone())
            })
            .transpose()?;
        let encoder = self.rhi_device.render_encoder.as_deref().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "Metal draw has no active render encoder",
            )
        })?;
        encoder.setRenderPipelineState(&pipeline);
        // SAFETY: preflight 已证明两个 buffer 角色、容量与 shader ABI 完全匹配。
        unsafe {
            encoder.setVertexBuffer_offset_atIndex(Some(&vertex), 0, 0);
            encoder.setVertexBuffer_offset_atIndex(Some(&uniform), 0, 1);
            encoder.setFragmentBuffer_offset_atIndex(Some(&uniform), 0, 1);
            if let Some((texture, sampler)) = &sampled {
                encoder.setFragmentTexture_atIndex(Some(texture), 0);
                encoder.setFragmentSamplerState_atIndex(Some(sampler), 0);
            }
        }
        let raster = packet.raster();
        let viewport = raster.viewport();
        encoder.setViewport(MTLViewport {
            originX: 0.0,
            originY: 0.0,
            width: viewport.width as f64,
            height: viewport.height as f64,
            znear: 0.0,
            zfar: 1.0,
        });
        let scissor = raster.scissor().unwrap_or(RhiScissor {
            x: 0,
            y: 0,
            width: viewport.width as i32,
            height: viewport.height as i32,
        });
        encoder.setScissorRect(metal_scissor(scissor));
        let range = packet.range();
        // SAFETY: DrawPacket 与真实资源的共同门禁已验证非零范围和字节边界。
        unsafe {
            if let Some(index) = index {
                encoder.drawIndexedPrimitives_indexCount_indexType_indexBuffer_indexBufferOffset(
                    MTLPrimitiveType::Triangle,
                    range.index_count() as usize,
                    MTLIndexType::UInt32,
                    &index,
                    range.first_index() as usize * size_of::<u32>(),
                );
            } else {
                encoder.drawPrimitives_vertexStart_vertexCount(
                    MTLPrimitiveType::Triangle,
                    range.first_vertex() as usize,
                    range.vertex_count() as usize,
                );
            }
        }
        Ok(())
    }

    pub(super) fn end_metal_render_pass(&mut self) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.pass.require_open()?;
        let encoder = self.rhi_device.render_encoder.take().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "Metal render pass has no active encoder",
            )
        })?;
        encoder.endEncoding();
        self.rhi_device.pass.end()?;
        self.rhi_device.active_target = None;
        self.rhi_device.active_format = None;
        Ok(())
    }
}

const fn metal_scissor(scissor: RhiScissor) -> MTLScissorRect {
    MTLScissorRect {
        x: scissor.x as usize,
        y: scissor.y as usize,
        width: scissor.width as usize,
        height: scissor.height as usize,
    }
}
