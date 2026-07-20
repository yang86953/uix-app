//! 可分离高斯模糊：水平 → 垂直两趟 fragment pass；大半径时先 2× 降采样再模糊上采样。
//!
//! Shader 内按 `sigma = radius / 3` 计算权重，与 CPU [`crate::draw::primitives::blur`] 对齐。

use std::mem::size_of;

use bytemuck::{Pod, Zeroable};

use crate::core::{Errc, Error, Rect, Result};
use crate::draw::primitives::blur::MAX_GPU_BLUR_KERNEL_RADIUS;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurUniforms {
    texel_size: [f32; 2],
    direction: [f32; 2],
    radius: f32,
    sigma: f32,
    /// uniform 结构体按 16 字节对齐填充。
    _pad: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

struct BlurScratch {
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    _texture: wgpu::Texture,
}

/// 共享 wgpu 可分离模糊管线与可复用中间纹理。
pub(super) struct SeparableBlur {
    pipeline: wgpu::RenderPipeline,
    sample_layout: wgpu::BindGroupLayout,
    blur_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buf: wgpu::Buffer,
    vertex_buf: wgpu::Buffer,
    blit_pipeline: wgpu::RenderPipeline,
    full_temp: Option<BlurScratch>,
    half_a: Option<BlurScratch>,
    half_b: Option<BlurScratch>,
    format: wgpu::TextureFormat,
}

impl SeparableBlur {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uix-separable-blur-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blur.wgsl").into()),
        });
        let sample_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uix-blur-sample-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let blur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uix-blur-conv-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("uix-blur-linear-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let attrs: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2];
        let replace = Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            write_mask: wgpu::ColorWrites::ALL,
        });
        let blur_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("uix-blur-conv-pipeline-layout"),
                bind_group_layouts: &[Some(&blur_layout)],
                immediate_size: 0,
            });
        let blit_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("uix-blur-blit-pipeline-layout"),
                bind_group_layouts: &[Some(&sample_layout)],
                immediate_size: 0,
            });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uix-separable-blur-pipeline"),
            layout: Some(&blur_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("blur_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<BlurVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attrs,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("blur_fs"),
                compilation_options: Default::default(),
                targets: &[replace.clone()],
            }),
            multiview_mask: None,
            cache: None,
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uix-blur-blit-pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("blur_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<BlurVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attrs,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("blit_fs"),
                compilation_options: Default::default(),
                targets: &[replace],
            }),
            multiview_mask: None,
            cache: None,
        });
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uix-blur-uniforms"),
            size: size_of::<BlurUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vertex_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uix-blur-vertices"),
            size: (size_of::<BlurVertex>() * 6) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            pipeline,
            sample_layout,
            blur_layout,
            sampler,
            uniform_buf,
            vertex_buf,
            blit_pipeline,
            full_temp: None,
            half_a: None,
            half_b: None,
            format,
        })
    }

    /// 对已 flush 的离屏颜色目标做可分离模糊。
    pub(super) fn blur_target(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        region: Rect,
        radius: f32,
    ) -> Result<()> {
        if !radius.is_finite() || radius < 0.5 || width == 0 || height == 0 {
            return Ok(());
        }
        let x0 = region.x.floor().clamp(0.0, width as f32) as u32;
        let y0 = region.y.floor().clamp(0.0, height as f32) as u32;
        let x1 = (region.x + region.w).ceil().clamp(0.0, width as f32) as u32;
        let y1 = (region.y + region.h).ceil().clamp(0.0, height as f32) as u32;
        if x0 >= x1 || y0 >= y1 {
            return Ok(());
        }
        let region_w = x1 - x0;
        let region_h = y1 - y0;
        let region_rect = Rect::new(x0 as f32, y0 as f32, region_w as f32, region_h as f32);

        let use_downsample =
            radius > MAX_GPU_BLUR_KERNEL_RADIUS as f32 && region_w >= 2 && region_h >= 2;
        if use_downsample {
            let half_w = region_w.div_ceil(2);
            let half_h = region_h.div_ceil(2);
            self.ensure_scratch(device, half_w, half_h, ScratchKind::HalfA)?;
            self.ensure_scratch(device, half_w, half_h, ScratchKind::HalfB)?;
            let half_a_view = self
                .half_a
                .as_ref()
                .map(|s| s.view.clone())
                .ok_or_else(|| Error::new(Errc::InvalidState, "wgpu blur half_a missing"))?;
            let half_b_view = self
                .half_b
                .as_ref()
                .map(|s| s.view.clone())
                .ok_or_else(|| Error::new(Errc::InvalidState, "wgpu blur half_b missing"))?;
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("uix-blur-downsample"),
            });
            self.encode_blit(
                device,
                queue,
                &mut encoder,
                source_view,
                width,
                height,
                region_rect,
                &half_a_view,
                half_w,
                half_h,
                Rect::new(0.0, 0.0, half_w as f32, half_h as f32),
            )?;
            let half_radius = (radius * 0.5).min(MAX_GPU_BLUR_KERNEL_RADIUS as f32);
            self.encode_conv(
                device,
                queue,
                &mut encoder,
                &half_a_view,
                &half_b_view,
                half_w,
                half_h,
                Rect::new(0.0, 0.0, half_w as f32, half_h as f32),
                half_radius,
                [1.0, 0.0],
            )?;
            self.encode_conv(
                device,
                queue,
                &mut encoder,
                &half_b_view,
                &half_a_view,
                half_w,
                half_h,
                Rect::new(0.0, 0.0, half_w as f32, half_h as f32),
                half_radius,
                [0.0, 1.0],
            )?;
            self.encode_blit(
                device,
                queue,
                &mut encoder,
                &half_a_view,
                half_w,
                half_h,
                Rect::new(0.0, 0.0, half_w as f32, half_h as f32),
                source_view,
                width,
                height,
                region_rect,
            )?;
            queue.submit(std::iter::once(encoder.finish()));
            return Ok(());
        }

        let blur_radius = radius.min(MAX_GPU_BLUR_KERNEL_RADIUS as f32);
        self.ensure_scratch(device, width, height, ScratchKind::Full)?;
        let temp_view = self
            .full_temp
            .as_ref()
            .map(|s| s.view.clone())
            .ok_or_else(|| Error::new(Errc::InvalidState, "wgpu blur full temp missing"))?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uix-blur-separable"),
        });
        self.encode_conv(
            device,
            queue,
            &mut encoder,
            source_view,
            &temp_view,
            width,
            height,
            region_rect,
            blur_radius,
            [1.0, 0.0],
        )?;
        self.encode_conv(
            device,
            queue,
            &mut encoder,
            &temp_view,
            source_view,
            width,
            height,
            region_rect,
            blur_radius,
            [0.0, 1.0],
        )?;
        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    fn encode_conv(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
        dest_view: &wgpu::TextureView,
        width: u32,
        height: u32,
        region: Rect,
        radius: f32,
        direction: [f32; 2],
    ) -> Result<()> {
        let uniforms = BlurUniforms {
            texel_size: [1.0 / width as f32, 1.0 / height as f32],
            direction,
            radius,
            sigma: radius / 3.0,
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
        self.write_dest_quad(queue, width, height, region, width, height, region);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uix-blur-conv-bind"),
            layout: &self.blur_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform_buf.as_entire_binding(),
                },
            ],
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("uix-blur-conv-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: dest_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buf.slice(..));
            set_region_scissor(&mut pass, width, height, region);
            pass.draw(0..6, 0..1);
        }
        Ok(())
    }

    fn encode_blit(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
        source_w: u32,
        source_h: u32,
        src: Rect,
        dest_view: &wgpu::TextureView,
        dest_w: u32,
        dest_h: u32,
        dst: Rect,
    ) -> Result<()> {
        self.write_dest_quad(queue, dest_w, dest_h, dst, source_w, source_h, src);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uix-blur-blit-bind"),
            layout: &self.sample_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("uix-blur-blit-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: dest_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buf.slice(..));
            set_region_scissor(&mut pass, dest_w, dest_h, dst);
            pass.draw(0..6, 0..1);
        }
        Ok(())
    }

    fn write_dest_quad(
        &self,
        queue: &wgpu::Queue,
        dest_w: u32,
        dest_h: u32,
        dest_region: Rect,
        src_w: u32,
        src_h: u32,
        src_region: Rect,
    ) {
        let x0 = dest_region.x;
        let y0 = dest_region.y;
        let x1 = dest_region.x + dest_region.w;
        let y1 = dest_region.y + dest_region.h;
        let sx0 = src_region.x;
        let sy0 = src_region.y;
        let sx1 = src_region.x + src_region.w;
        let sy1 = src_region.y + src_region.h;
        let ndc = |x: f32, y: f32| -> [f32; 2] {
            [
                (x / dest_w as f32) * 2.0 - 1.0,
                1.0 - (y / dest_h as f32) * 2.0,
            ]
        };
        let uv = |x: f32, y: f32| -> [f32; 2] { [x / src_w as f32, y / src_h as f32] };
        let verts = [
            BlurVertex {
                position: ndc(x0, y0),
                uv: uv(sx0, sy0),
            },
            BlurVertex {
                position: ndc(x1, y0),
                uv: uv(sx1, sy0),
            },
            BlurVertex {
                position: ndc(x1, y1),
                uv: uv(sx1, sy1),
            },
            BlurVertex {
                position: ndc(x0, y0),
                uv: uv(sx0, sy0),
            },
            BlurVertex {
                position: ndc(x1, y1),
                uv: uv(sx1, sy1),
            },
            BlurVertex {
                position: ndc(x0, y1),
                uv: uv(sx0, sy1),
            },
        ];
        queue.write_buffer(&self.vertex_buf, 0, bytemuck::cast_slice(&verts));
    }

    fn ensure_scratch(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        kind: ScratchKind,
    ) -> Result<()> {
        let slot = match kind {
            ScratchKind::Full => &mut self.full_temp,
            ScratchKind::HalfA => &mut self.half_a,
            ScratchKind::HalfB => &mut self.half_b,
        };
        if let Some(existing) = slot.as_ref() {
            if existing.width == width && existing.height == height {
                return Ok(());
            }
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(match kind {
                ScratchKind::Full => "uix-blur-full-temp",
                ScratchKind::HalfA => "uix-blur-half-a",
                ScratchKind::HalfB => "uix-blur-half-b",
            }),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        *slot = Some(BlurScratch {
            view,
            width,
            height,
            _texture: texture,
        });
        Ok(())
    }
}

enum ScratchKind {
    Full,
    HalfA,
    HalfB,
}

fn set_region_scissor(pass: &mut wgpu::RenderPass<'_>, width: u32, height: u32, region: Rect) {
    let x = region.x.max(0.0) as u32;
    let y = region.y.max(0.0) as u32;
    let w = region.w.max(0.0) as u32;
    let h = region.h.max(0.0) as u32;
    if w == 0 || h == 0 || x >= width || y >= height {
        return;
    }
    pass.set_scissor_rect(
        x,
        y,
        w.min(width.saturating_sub(x)),
        h.min(height.saturating_sub(y)),
    );
}
