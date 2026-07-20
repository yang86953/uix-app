//! One GPU renderer shared by every wgpu backend.

use std::mem::size_of;

use crate::core::{Errc, Error, Rect, Result};
use crate::native::graphics::wgpu_backend::draw_stream::{
    align_up, align_up_u32, ndc, quad_positions, BatchKind, DrawStream, GlyphVertex, ShapeVertex,
    TextureVertex, COPY_BYTES_PER_ROW_ALIGNMENT, GLYPH_ATLAS_HEIGHT, GLYPH_ATLAS_WIDTH,
};
use crate::native::graphics::wgpu_backend::glyph_batch;
use crate::native::graphics::wgpu_backend::glyph_cover::GlyphCoverPipeline;
use crate::native::traits::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuImageBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh,
    GpuSolidRect, GpuStrokeRect,
};

pub(super) use crate::native::graphics::wgpu_backend::draw_stream::ActiveTarget;

/// 帧间保留的主色缓冲：绘制写入此处，present 再全幅复制到 swapchain。
struct RetainedColorTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

/// 全屏 overlay 动画用的干净主表面 GPU 快照（无 CPU readback）。
struct OverlayBackdropTarget {
    texture: wgpu::Texture,
    width: u32,
    height: u32,
}

pub(super) struct WgpuRenderer {
    shape_pipeline: wgpu::RenderPipeline,
    clear_pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    glyph_msdf_pipeline: wgpu::RenderPipeline,
    texture_pipeline: wgpu::RenderPipeline,
    /// 父画布 Additive：src One + dst One，对齐 CPU 通道相加。
    texture_additive_pipeline: wgpu::RenderPipeline,
    glyph_texture: wgpu::Texture,
    /// R8 coverage atlas 视图；bind group 持有克隆，字段保活供后续维护。
    #[allow(dead_code)]
    glyph_atlas_view: wgpu::TextureView,
    glyph_bind_group: wgpu::BindGroup,
    /// 轮廓字形 MSDF atlas（RGBA8）；与 R8 coverage atlas 并存。
    /// 纹理本体由 view / bind group 引用；字段保证与 renderer 同寿。
    #[allow(dead_code)]
    msdf_texture: wgpu::Texture,
    msdf_atlas_view: wgpu::TextureView,
    msdf_bind_group: wgpu::BindGroup,
    glyph_cover: GlyphCoverPipeline,
    texture_bind_layout: wgpu::BindGroupLayout,
    texture_sampler: wgpu::Sampler,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    swapchain: DrawStream,
    offscreen: DrawStream,
    active: ActiveTarget,
    /// Bind groups kept alive until the owning stream flushes / presents.
    frame_bind_groups: Vec<wgpu::BindGroup>,
    /// Uploaded image textures kept alive until present.
    frame_textures: Vec<wgpu::Texture>,
    surface_format: wgpu::TextureFormat,
    /// 主路径保留色缓冲；尺寸与 drawable 对齐，resize 时丢弃。
    retained_color: Option<RetainedColorTarget>,
    /// Overlay 打开前捕获的干净主色缓冲；动画帧 restore 后再只绘浮层。
    overlay_backdrop: Option<OverlayBackdropTarget>,
}

impl WgpuRenderer {
    pub(super) fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uix-shared-2d-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("renderer.wgsl").into()),
        });
        let glyph_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uix-glyph-atlas"),
            size: wgpu::Extent3d {
                width: GLYPH_ATLAS_WIDTH,
                height: GLYPH_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let glyph_atlas_view = glyph_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let glyph_view = glyph_atlas_view.clone();
        let glyph_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("uix-glyph-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let glyph_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uix-glyph-layout"),
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
        let glyph_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uix-glyph-bind-group"),
            layout: &glyph_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&glyph_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&glyph_sampler),
                },
            ],
        });
        let msdf_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uix-glyph-msdf-atlas"),
            size: wgpu::Extent3d {
                width: GLYPH_ATLAS_WIDTH,
                height: GLYPH_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let msdf_atlas_view = msdf_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let msdf_view = msdf_atlas_view.clone();
        let msdf_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uix-glyph-msdf-bind-group"),
            layout: &glyph_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&msdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&glyph_sampler),
                },
            ],
        });
        let texture_bind_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uix-texture-layout"),
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
        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("uix-texture-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let shape_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("uix-shape-pipeline-layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let glyph_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("uix-glyph-pipeline-layout"),
                bind_group_layouts: &[Some(&glyph_layout)],
                immediate_size: 0,
            });
        let texture_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("uix-texture-pipeline-layout"),
                bind_group_layouts: &[Some(&texture_bind_layout)],
                immediate_size: 0,
            });
        let target = Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        });
        const SHAPE_ATTRIBUTES: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x4,
            3 => Float32x4,
            4 => Float32x4,
            5 => Float32x4,
            6 => Uint32
        ];
        const GLYPH_ATTRIBUTES: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];
        // stride 32：position(8)+uv(8)+opacity(4)+pad(12)，与 TextureVertex 一致。
        const TEXTURE_ATTRIBUTES: [wgpu::VertexAttribute; 3] = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 8,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 16,
                shader_location: 2,
            },
        ];
        let shape_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uix-shape-pipeline"),
            layout: Some(&shape_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("shape_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<ShapeVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &SHAPE_ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("shape_fs"),
                compilation_options: Default::default(),
                targets: &[target.clone()],
            }),
            multiview_mask: None,
            cache: None,
        });
        let clear_target = Some(wgpu::ColorTargetState {
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
        let clear_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uix-clear-pipeline"),
            layout: Some(&shape_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("shape_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<ShapeVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &SHAPE_ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("shape_fs"),
                compilation_options: Default::default(),
                targets: &[clear_target],
            }),
            multiview_mask: None,
            cache: None,
        });
        let glyph_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uix-glyph-pipeline"),
            layout: Some(&glyph_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("glyph_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<GlyphVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &GLYPH_ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("glyph_fs"),
                compilation_options: Default::default(),
                targets: &[target.clone()],
            }),
            multiview_mask: None,
            cache: None,
        });
        let glyph_msdf_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uix-glyph-msdf-pipeline"),
            layout: Some(&glyph_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("glyph_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<GlyphVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &GLYPH_ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("glyph_msdf_fs"),
                compilation_options: Default::default(),
                targets: &[target.clone()],
            }),
            multiview_mask: None,
            cache: None,
        });
        let texture_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uix-texture-pipeline"),
            layout: Some(&texture_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("texture_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<TextureVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &TEXTURE_ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("texture_fs"),
                compilation_options: Default::default(),
                targets: &[target.clone()],
            }),
            multiview_mask: None,
            cache: None,
        });
        let additive_target = Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            write_mask: wgpu::ColorWrites::ALL,
        });
        let texture_additive_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("uix-texture-additive-pipeline"),
                layout: Some(&texture_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("texture_vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: size_of::<TextureVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &TEXTURE_ATTRIBUTES,
                    })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("texture_fs"),
                    compilation_options: Default::default(),
                    targets: &[additive_target],
                }),
                multiview_mask: None,
                cache: None,
            });
        let vertex_capacity = 4096;
        let vertex_buffer = create_vertex_buffer(device, vertex_capacity);
        let glyph_cover = GlyphCoverPipeline::new(device)?;
        Ok(Self {
            shape_pipeline,
            clear_pipeline,
            glyph_pipeline,
            glyph_msdf_pipeline,
            texture_pipeline,
            texture_additive_pipeline,
            glyph_texture,
            glyph_atlas_view,
            glyph_bind_group,
            msdf_texture,
            msdf_atlas_view,
            msdf_bind_group,
            glyph_cover,
            texture_bind_layout,
            texture_sampler,
            vertex_buffer,
            vertex_capacity,
            swapchain: DrawStream::new(),
            offscreen: DrawStream::new(),
            active: ActiveTarget::Swapchain,
            frame_bind_groups: Vec::new(),
            frame_textures: Vec::new(),
            surface_format: format,
            retained_color: None,
            overlay_backdrop: None,
        })
    }

    pub(super) fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_format
    }

    pub(super) fn texture_bind_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_bind_layout
    }

    pub(super) fn texture_sampler(&self) -> &wgpu::Sampler {
        &self.texture_sampler
    }

    pub(super) fn active_target(&self) -> ActiveTarget {
        self.active
    }

    pub(super) fn set_active_target(&mut self, target: ActiveTarget) {
        self.active = target;
    }

    pub(super) fn begin_frame(&mut self, clear: [f32; 4]) {
        match self.active {
            ActiveTarget::Swapchain => {
                self.swapchain.reset(clear);
                self.frame_bind_groups.clear();
                self.frame_textures.clear();
            }
            ActiveTarget::Offscreen => self.offscreen.reset(clear),
        }
    }

    fn stream(&mut self) -> &mut DrawStream {
        match self.active {
            ActiveTarget::Swapchain => &mut self.swapchain,
            ActiveTarget::Offscreen => &mut self.offscreen,
        }
    }

    pub(super) fn solid_rects(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) {
        for rect in rects {
            self.shape_quad(
                viewport,
                scissor,
                Rect::new(rect.x, rect.y, rect.w, rect.h),
                rect.rgba,
                rect.rgba,
                [rect.w, rect.h, 0.0, 0.0],
                rect.radius,
                u32::from(rect.radius.iter().any(|radius| *radius > 0.0)),
                LocalCoordinates::Pixels,
                BatchKind::Shape,
            );
        }
    }

    /// 脏区 replace 清除：透明色覆写，不走 SrcOver。
    pub(super) fn clear_rects(
        &mut self,
        viewport: (f32, f32),
        rects: &[GpuSolidRect],
    ) {
        for rect in rects {
            self.shape_quad(
                viewport,
                None,
                Rect::new(rect.x, rect.y, rect.w, rect.h),
                rect.rgba,
                rect.rgba,
                [rect.w, rect.h, 0.0, 0.0],
                [0.0; 4],
                0,
                LocalCoordinates::Pixels,
                BatchKind::Clear,
            );
        }
    }

    pub(super) fn stroke_rects(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuStrokeRect],
    ) {
        for rect in rects {
            let half = rect.line_width * 0.5;
            self.shape_quad(
                viewport,
                scissor,
                Rect::new(
                    rect.x - half,
                    rect.y - half,
                    rect.w + rect.line_width,
                    rect.h + rect.line_width,
                ),
                rect.rgba,
                rect.rgba,
                [
                    rect.w + rect.line_width,
                    rect.h + rect.line_width,
                    rect.line_width,
                    0.0,
                ],
                rect.radius.map(|radius| radius + half),
                2,
                LocalCoordinates::Pixels,
                BatchKind::Shape,
            );
        }
    }

    pub(super) fn linear_gradients(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuLinearGradientRect],
    ) {
        for rect in rects {
            self.shape_quad(
                viewport,
                scissor,
                Rect::new(rect.x, rect.y, rect.w, rect.h),
                rect.color_a,
                rect.color_b,
                [rect.dir as f32, 0.0, 0.0, 0.0],
                [0.0; 4],
                3,
                LocalCoordinates::Normalized,
                BatchKind::Shape,
            );
        }
    }

    pub(super) fn radial_gradients(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        gradients: &[GpuRadialGradient],
    ) {
        for gradient in gradients {
            let radius = gradient.outer_r;
            self.shape_quad(
                viewport,
                scissor,
                Rect::new(
                    gradient.cx - radius,
                    gradient.cy - radius,
                    radius * 2.0,
                    radius * 2.0,
                ),
                gradient.color_inner,
                gradient.color_outer,
                [gradient.inner_r, gradient.outer_r, 0.0, 0.0],
                [0.0; 4],
                4,
                LocalCoordinates::Centered,
                BatchKind::Shape,
            );
        }
    }

    pub(super) fn solid_meshes(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        meshes: &[GpuSolidMesh],
    ) {
        for mesh in meshes {
            let stream = self.stream();
            let first = stream.shapes.len() as u32;
            for point in mesh.vertices.chunks_exact(2) {
                stream.shapes.push(ShapeVertex {
                    position: ndc(point[0], point[1], viewport),
                    local: [0.0; 2],
                    color_a: mesh.rgba,
                    color_b: mesh.rgba,
                    params0: [0.0; 4],
                    params1: [0.0; 4],
                    mode: 0,
                });
            }
            let count = stream.shapes.len() as u32 - first;
            stream.push_batch(BatchKind::Shape, first, count, viewport, scissor);
        }
    }

    pub(super) fn box_shadows(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        shadows: &[GpuBoxShadow],
    ) {
        for shadow in shadows {
            let blur_x = shadow.blur_x.max(0.5);
            let blur_y = shadow.blur_y.max(0.5);
            let mode = if shadow.ambient { 6 } else { 5 };
            let inflate = (blur_x + blur_y) * 0.5;
            let expand_w = shadow.w + blur_x * 2.0;
            let expand_h = shadow.h + blur_y * 2.0;
            self.shape_oriented_quad(
                viewport,
                scissor,
                shadow.corners,
                [
                    [0.0, 0.0],
                    [expand_w, 0.0],
                    [expand_w, expand_h],
                    [0.0, expand_h],
                ],
                shadow.rgba,
                shadow.rgba,
                [expand_w, expand_h, blur_x, blur_y],
                shadow.radius.map(|radius| radius + inflate),
                mode,
                BatchKind::Shape,
            );
        }
    }

    pub(super) fn glyphs(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        glyph_batch::record_glyphs(self.stream(), viewport, scissor, glyphs)
    }

    pub(super) fn queue_sampled_blit(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        bind_group: wgpu::BindGroup,
        src: Rect,
        source_size: (f32, f32),
        dst: Rect,
        opacity: f32,
        additive: bool,
    ) {
        if dst.w <= 0.0 || dst.h <= 0.0 || source_size.0 <= 0.0 || source_size.1 <= 0.0 {
            return;
        }
        let bind_index = self.frame_bind_groups.len() as u32;
        self.frame_bind_groups.push(bind_group);
        let u0 = (src.x / source_size.0).clamp(0.0, 1.0);
        let v0 = (src.y / source_size.1).clamp(0.0, 1.0);
        let u1 = ((src.x + src.w) / source_size.0).clamp(0.0, 1.0);
        let v1 = ((src.y + src.h) / source_size.1).clamp(0.0, 1.0);
        let stream = self.stream();
        let first = stream.textures.len() as u32;
        let positions = quad_positions(dst);
        let uvs = [[u0, v0], [u1, v0], [u1, v1], [u0, v0], [u1, v1], [u0, v1]];
        let opacity = opacity.clamp(0.0, 1.0);
        for (position, uv) in positions.into_iter().zip(uvs) {
            stream.textures.push(TextureVertex {
                position: ndc(position[0], position[1], viewport),
                uv,
                opacity,
                _pad: [0.0; 3],
            });
        }
        stream.push_batch(
            BatchKind::Texture {
                bind_group: bind_index,
                additive,
            },
            first,
            6,
            viewport,
            scissor,
        );
    }

    pub(super) fn image_blits(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        blits: &[GpuImageBlit],
    ) -> Result<()> {
        for blit in blits {
            if blit.pixel_w == 0 || blit.pixel_h == 0 || blit.w <= 0.0 || blit.h <= 0.0 {
                continue;
            }
            let expected = (blit.pixel_w as usize).saturating_mul(blit.pixel_h as usize);
            if blit.pixels.len() < expected {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    "wgpu image blit pixel buffer is smaller than pixel_w * pixel_h",
                ));
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("uix-image-blit"),
                size: wgpu::Extent3d {
                    width: blit.pixel_w,
                    height: blit.pixel_h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            write_bgra_texture(
                queue,
                &texture,
                blit.pixel_w,
                blit.pixel_h,
                &blit.pixels[..expected],
            )?;
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("uix-image-blit-bind-group"),
                layout: &self.texture_bind_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.texture_sampler),
                    },
                ],
            });
            self.frame_textures.push(texture);
            self.queue_sampled_blit(
                viewport,
                scissor,
                bind_group,
                Rect::new(0.0, 0.0, blit.pixel_w as f32, blit.pixel_h as f32),
                (blit.pixel_w as f32, blit.pixel_h as f32),
                Rect::new(blit.x, blit.y, blit.w, blit.h),
                blit.opacity,
                blit.additive,
            );
        }
        Ok(())
    }

    /// Encode the offscreen command stream into `view` and submit.
    pub(super) fn flush_offscreen_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> Result<()> {
        if self.offscreen.is_empty() {
            return Ok(());
        }
        self.flush_stream_to_view(device, queue, ActiveTarget::Offscreen, view, width, height)?;
        self.offscreen.clear_commands_only();
        Ok(())
    }

    pub(super) fn present(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface: &wgpu::Surface<'_>,
        width: u32,
        height: u32,
    ) -> Result<()> {
        if self.active != ActiveTarget::Swapchain {
            return Err(Error::new(
                Errc::InvalidState,
                "wgpu present requires the swapchain target",
            ));
        }
        let width = width.max(1);
        let height = height.max(1);
        // 先绘入保留色缓冲，再全幅复制到 swapchain：绘制侧可 Load/脏区 clear，
        // present 仍保持 FullOnly（不依赖 swapchain 图像保留）。
        self.ensure_retained_color(device, width, height)?;
        let (retained_view, retained_texture) = {
            let retained = self.retained_color.as_ref().ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "wgpu retained color target missing after ensure",
                )
            })?;
            (retained.view.clone(), retained.texture.clone())
        };
        self.flush_stream_to_view(
            device,
            queue,
            ActiveTarget::Swapchain,
            &retained_view,
            width,
            height,
        )?;

        let surface_texture = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Err(Error::new(
                    Errc::GraphicsOccluded,
                    "wgpu surface is temporarily unavailable",
                ));
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                return Err(Error::new(
                    Errc::GraphicsSurfaceLost,
                    "wgpu surface must be reconfigured",
                ));
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(Error::new(
                    Errc::PlatformError,
                    "wgpu surface validation failed",
                ));
            }
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uix-present-copy"),
        });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &retained_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &surface_texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));
        queue.present(surface_texture);
        self.swapchain.clear_commands_only();
        self.frame_bind_groups.clear();
        self.frame_textures.clear();
        Ok(())
    }

    /// resize / surface rebuild 后丢弃保留缓冲，下一帧强制全幅 clear。
    pub(super) fn invalidate_retained_color(&mut self) {
        self.retained_color = None;
        self.overlay_backdrop = None;
        self.swapchain.needs_clear = true;
    }

    /// 是否持有干净的 overlay 主表面 GPU 快照。
    pub(super) fn has_overlay_backdrop(&self) -> bool {
        self.overlay_backdrop.is_some()
    }

    /// 释放 overlay 背景快照（普通树变脏、尺寸变化或浮层全部离场）。
    pub(super) fn release_overlay_backdrop(&mut self) {
        self.overlay_backdrop = None;
    }

    /// 在 begin_frame 清除前，把保留色缓冲复制到 overlay 快照（无 CPU readback）。
    pub(super) fn snapshot_overlay_backdrop(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        let (retained_tex, width, height) = {
            let retained = self.retained_color.as_ref().ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "wgpu overlay backdrop snapshot requires a retained color target",
                )
            })?;
            (
                retained.texture.clone(),
                retained.width,
                retained.height,
            )
        };
        if width == 0 || height == 0 {
            return Err(Error::new(
                Errc::InvalidState,
                "wgpu overlay backdrop snapshot rejected empty retained extent",
            ));
        }
        let needs_new = match &self.overlay_backdrop {
            Some(target) => target.width != width || target.height != height,
            None => true,
        };
        if needs_new {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("uix-overlay-backdrop"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_format,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.overlay_backdrop = Some(OverlayBackdropTarget {
                texture,
                width,
                height,
            });
        }
        let backdrop = self.overlay_backdrop.as_ref().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "wgpu overlay backdrop missing after ensure",
            )
        })?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uix-overlay-backdrop-snapshot"),
        });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &retained_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &backdrop.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// 把 overlay 快照写回保留色缓冲，并取消本帧全幅 LoadOp::Clear。
    ///
    /// 调用方须在 begin_frame 标记 clear 之后、绘制浮层之前调用，使半透明
    /// 遮罩每帧都从干净背景合成，而不是叠在上一帧的遮罩上。
    pub(super) fn restore_overlay_backdrop(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        let (backdrop_tex, width, height) = {
            let backdrop = self.overlay_backdrop.as_ref().ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "wgpu overlay backdrop restore requires a captured snapshot",
                )
            })?;
            (
                backdrop.texture.clone(),
                backdrop.width,
                backdrop.height,
            )
        };
        // 恢复路径不得重建保留缓冲：ensure 在新建时会清掉 overlay_backdrop。
        let retained = self.retained_color.as_ref().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "wgpu overlay backdrop restore requires an existing retained color target",
            )
        })?;
        if retained.width != width || retained.height != height {
            return Err(Error::new(
                Errc::InvalidState,
                "wgpu overlay backdrop extent does not match retained color",
            ));
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uix-overlay-backdrop-restore"),
        });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &backdrop_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &retained.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));
        // 快照已写入保留缓冲：后续 flush 必须 Load，不能 Clear 抹掉背景。
        self.swapchain.mark_content_retained();
        Ok(())
    }

    fn ensure_retained_color(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let needs_new = match &self.retained_color {
            Some(target) => target.width != width || target.height != height,
            None => true,
        };
        if needs_new {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("uix-retained-color"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.retained_color = Some(RetainedColorTarget {
                texture,
                view,
                width,
                height,
            });
            // 新缓冲内容未定义，必须全幅 clear 后再绘制。
            self.swapchain.needs_clear = true;
            // 尺寸变化后旧快照不再有效。
            self.overlay_backdrop = None;
        }
        Ok(())
    }

    fn flush_stream_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        which: ActiveTarget,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let covers = {
            let stream = match which {
                ActiveTarget::Swapchain => &mut self.swapchain,
                ActiveTarget::Offscreen => &mut self.offscreen,
            };
            if stream.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut stream.glyph_covers)
        };
        let stream = match which {
            ActiveTarget::Swapchain => &self.swapchain,
            ActiveTarget::Offscreen => &self.offscreen,
        };
        let shape_bytes = bytemuck::cast_slice(&stream.shapes);
        let glyph_bytes = bytemuck::cast_slice(&stream.glyphs);
        let msdf_bytes = bytemuck::cast_slice(&stream.msdf_glyphs);
        let texture_bytes = bytemuck::cast_slice(&stream.textures);
        let glyph_offset = align_up(shape_bytes.len() as u64, 16);
        let msdf_offset = align_up(
            glyph_offset.saturating_add(glyph_bytes.len() as u64),
            16,
        );
        let texture_offset = align_up(
            msdf_offset.saturating_add(msdf_bytes.len() as u64),
            16,
        );
        let required = texture_offset
            .saturating_add(texture_bytes.len() as u64)
            .max(1);
        if required > self.vertex_capacity {
            self.vertex_capacity = required.next_power_of_two();
            self.vertex_buffer = create_vertex_buffer(device, self.vertex_capacity);
        }
        if !shape_bytes.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, shape_bytes);
        }
        if !glyph_bytes.is_empty() {
            queue.write_buffer(&self.vertex_buffer, glyph_offset, glyph_bytes);
            if stream.glyph_used_height > 0 {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.glyph_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &stream.glyph_pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(GLYPH_ATLAS_WIDTH),
                        rows_per_image: Some(stream.glyph_used_height),
                    },
                    wgpu::Extent3d {
                        width: GLYPH_ATLAS_WIDTH,
                        height: stream.glyph_used_height,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        if !msdf_bytes.is_empty() {
            queue.write_buffer(&self.vertex_buffer, msdf_offset, msdf_bytes);
        }
        if !texture_bytes.is_empty() {
            queue.write_buffer(&self.vertex_buffer, texture_offset, texture_bytes);
        }
        let needs_clear = stream.needs_clear;
        let clear = stream.clear;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uix-frame-encoder"),
        });
        // 先把轮廓边写入 RGBA8 MSDF atlas，再进入主 pass 采样。
        if !covers.is_empty() {
            self.glyph_cover.encode(
                device,
                queue,
                &mut encoder,
                &self.msdf_atlas_view,
                GLYPH_ATLAS_WIDTH,
                GLYPH_ATLAS_HEIGHT,
                &covers,
            )?;
        }
        let load = if needs_clear {
            wgpu::LoadOp::Clear(wgpu::Color {
                r: clear[0] as f64,
                g: clear[1] as f64,
                b: clear[2] as f64,
                a: clear[3] as f64,
            })
        } else {
            wgpu::LoadOp::Load
        };
        let stream = match which {
            ActiveTarget::Swapchain => &self.swapchain,
            ActiveTarget::Offscreen => &self.offscreen,
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("uix-main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut bound: Option<BatchKind> = None;
            for batch in &stream.batches {
                if bound.as_ref().map(|kind| match (kind, &batch.kind) {
                    (BatchKind::Shape, BatchKind::Shape)
                    | (BatchKind::Clear, BatchKind::Clear)
                    | (BatchKind::Glyph, BatchKind::Glyph)
                    | (BatchKind::GlyphMsdf, BatchKind::GlyphMsdf) => true,
                    (
                        BatchKind::Texture {
                            bind_group: a,
                            additive: add_a,
                        },
                        BatchKind::Texture {
                            bind_group: b,
                            additive: add_b,
                        },
                    ) => a == b && add_a == add_b,
                    _ => false,
                }) != Some(true)
                {
                    match batch.kind {
                        BatchKind::Shape => {
                            pass.set_pipeline(&self.shape_pipeline);
                            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..glyph_offset));
                        }
                        BatchKind::Clear => {
                            pass.set_pipeline(&self.clear_pipeline);
                            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..glyph_offset));
                        }
                        BatchKind::Glyph => {
                            pass.set_pipeline(&self.glyph_pipeline);
                            pass.set_bind_group(0, &self.glyph_bind_group, &[]);
                            pass.set_vertex_buffer(
                                0,
                                self.vertex_buffer.slice(glyph_offset..msdf_offset),
                            );
                        }
                        BatchKind::GlyphMsdf => {
                            pass.set_pipeline(&self.glyph_msdf_pipeline);
                            pass.set_bind_group(0, &self.msdf_bind_group, &[]);
                            pass.set_vertex_buffer(
                                0,
                                self.vertex_buffer.slice(msdf_offset..texture_offset),
                            );
                        }
                        BatchKind::Texture {
                            bind_group,
                            additive,
                        } => {
                            if additive {
                                pass.set_pipeline(&self.texture_additive_pipeline);
                            } else {
                                pass.set_pipeline(&self.texture_pipeline);
                            }
                            let group = self
                                .frame_bind_groups
                                .get(bind_group as usize)
                                .ok_or_else(|| {
                                    Error::new(
                                        Errc::InvalidState,
                                        "wgpu texture blit bind group missing",
                                    )
                                })?;
                            pass.set_bind_group(0, group, &[]);
                            pass.set_vertex_buffer(0, self.vertex_buffer.slice(texture_offset..));
                        }
                    }
                    bound = Some(batch.kind);
                }
                let (x, y, scissor_width, scissor_height) =
                    physical_scissor((width, height), batch.viewport, batch.scissor);
                if scissor_width == 0 || scissor_height == 0 {
                    continue;
                }
                pass.set_scissor_rect(x, y, scissor_width, scissor_height);
                pass.draw(
                    batch.first_vertex..batch.first_vertex + batch.vertex_count,
                    0..1,
                );
            }
        }
        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn shape_oriented_quad(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        corners: [[f32; 2]; 4],
        locals: [[f32; 2]; 4],
        color_a: [f32; 4],
        color_b: [f32; 4],
        params0: [f32; 4],
        params1: [f32; 4],
        mode: u32,
        kind: BatchKind,
    ) {
        let stream = self.stream();
        let first = stream.shapes.len() as u32;
        let positions = [
            corners[0], corners[1], corners[2], corners[0], corners[2], corners[3],
        ];
        let local = [
            locals[0], locals[1], locals[2], locals[0], locals[2], locals[3],
        ];
        for (position, local) in positions.into_iter().zip(local) {
            stream.shapes.push(ShapeVertex {
                position: ndc(position[0], position[1], viewport),
                local,
                color_a,
                color_b,
                params0,
                params1,
                mode,
            });
        }
        stream.push_batch(kind, first, 6, viewport, scissor);
    }

    #[allow(clippy::too_many_arguments)]
    fn shape_quad(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        rect: Rect,
        color_a: [f32; 4],
        color_b: [f32; 4],
        params0: [f32; 4],
        params1: [f32; 4],
        mode: u32,
        coordinates: LocalCoordinates,
        kind: BatchKind,
    ) {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let stream = self.stream();
        let first = stream.shapes.len() as u32;
        let positions = quad_positions(rect);
        let local = coordinates.values(rect);
        for (position, local) in positions.into_iter().zip(local) {
            stream.shapes.push(ShapeVertex {
                position: ndc(position[0], position[1], viewport),
                local,
                color_a,
                color_b,
                params0,
                params1,
                mode,
            });
        }
        stream.push_batch(kind, first, 6, viewport, scissor);
    }
}

#[derive(Clone, Copy)]
enum LocalCoordinates {
    Pixels,
    Normalized,
    Centered,
}

impl LocalCoordinates {
    fn values(self, rect: Rect) -> [[f32; 2]; 6] {
        match self {
            Self::Pixels => [
                [0.0, 0.0],
                [rect.w, 0.0],
                [rect.w, rect.h],
                [0.0, 0.0],
                [rect.w, rect.h],
                [0.0, rect.h],
            ],
            Self::Normalized => [
                [0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
            ],
            Self::Centered => {
                let radius = rect.w * 0.5;
                [
                    [-radius, -radius],
                    [radius, -radius],
                    [radius, radius],
                    [-radius, -radius],
                    [radius, radius],
                    [-radius, radius],
                ]
            }
        }
    }
}

fn create_vertex_buffer(device: &wgpu::Device, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("uix-shared-vertex-buffer"),
        size,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn write_bgra_texture(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    pixels: &[u32],
) -> Result<()> {
    let unpadded = width.saturating_mul(4);
    let padded = align_up_u32(unpadded, COPY_BYTES_PER_ROW_ALIGNMENT);
    let source = bytemuck::cast_slice::<u32, u8>(pixels);
    if padded == unpadded {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            source,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(unpadded),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        return Ok(());
    }
    let mut padded_bytes = Vec::new();
    padded_bytes
        .try_reserve_exact((padded as usize).saturating_mul(height as usize))
        .map_err(|error| {
            Error::new(
                Errc::GraphicsOutOfMemory,
                format!("wgpu image blit row padding allocation failed: {error}"),
            )
        })?;
    for row in 0..height as usize {
        let start = row * unpadded as usize;
        padded_bytes.extend_from_slice(&source[start..start + unpadded as usize]);
        padded_bytes.resize(padded_bytes.len() + (padded - unpadded) as usize, 0);
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &padded_bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(padded),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    Ok(())
}

fn physical_scissor(
    extent: (u32, u32),
    viewport: (f32, f32),
    scissor: Option<(i32, i32, i32, i32)>,
) -> (u32, u32, u32, u32) {
    let Some((x, y, width, height)) = scissor else {
        return (0, 0, extent.0, extent.1);
    };
    let scale_x = extent.0 as f32 / viewport.0.max(1.0);
    let scale_y = extent.1 as f32 / viewport.1.max(1.0);
    let left = ((x as f32 * scale_x).floor() as i64).clamp(0, extent.0 as i64) as u32;
    let top = ((y as f32 * scale_y).floor() as i64).clamp(0, extent.1 as i64) as u32;
    let right =
        (((x + width) as f32 * scale_x).ceil() as i64).clamp(left as i64, extent.0 as i64) as u32;
    let bottom =
        (((y + height) as f32 * scale_y).ceil() as i64).clamp(top as i64, extent.1 as i64) as u32;
    (left, top, right - left, bottom - top)
}
