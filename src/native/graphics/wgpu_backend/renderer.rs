//! One GPU renderer shared by every wgpu backend.

use std::{collections::HashMap, mem::size_of, sync::Arc};

use bytemuck::{Pod, Zeroable};

use crate::core::{Errc, Error, Rect, Result};
use crate::native::traits::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSolidMesh,
    GpuSolidRect, GpuStrokeRect,
};

const GLYPH_ATLAS_WIDTH: u32 = 1024;
const GLYPH_ATLAS_HEIGHT: u32 = 1024;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShapeVertex {
    position: [f32; 2],
    local: [f32; 2],
    color_a: [f32; 4],
    color_b: [f32; 4],
    params0: [f32; 4],
    params1: [f32; 4],
    mode: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlyphVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BatchKind {
    Shape,
    Glyph,
}

struct DrawBatch {
    kind: BatchKind,
    first_vertex: u32,
    vertex_count: u32,
    viewport: (f32, f32),
    scissor: Option<(i32, i32, i32, i32)>,
}

pub(super) struct WgpuRenderer {
    shape_pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    glyph_texture: wgpu::Texture,
    glyph_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    shapes: Vec<ShapeVertex>,
    glyphs: Vec<GlyphVertex>,
    batches: Vec<DrawBatch>,
    glyph_pixels: Vec<u8>,
    glyph_cursor_x: u32,
    glyph_cursor_y: u32,
    glyph_row_height: u32,
    glyph_used_height: u32,
    glyph_locations: HashMap<(usize, u32, u32), (u32, u32)>,
    // Keep atlas keys alive for the whole frame so allocator address reuse
    // cannot alias two different coverage buffers in `glyph_locations`.
    glyph_sources: Vec<Arc<[u8]>>,
    clear: [f32; 4],
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
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let glyph_view = glyph_texture.create_view(&wgpu::TextureViewDescriptor::default());
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
                targets: &[target],
            }),
            multiview_mask: None,
            cache: None,
        });
        let vertex_capacity = 4096;
        let vertex_buffer = create_vertex_buffer(device, vertex_capacity);
        Ok(Self {
            shape_pipeline,
            glyph_pipeline,
            glyph_texture,
            glyph_bind_group,
            vertex_buffer,
            vertex_capacity,
            shapes: Vec::new(),
            glyphs: Vec::new(),
            batches: Vec::new(),
            glyph_pixels: Vec::new(),
            glyph_cursor_x: 0,
            glyph_cursor_y: 0,
            glyph_row_height: 0,
            glyph_used_height: 0,
            glyph_locations: HashMap::new(),
            glyph_sources: Vec::new(),
            clear: [0.0; 4],
        })
    }

    pub(super) fn begin_frame(&mut self, clear: [f32; 4]) {
        self.shapes.clear();
        self.glyphs.clear();
        self.batches.clear();
        self.glyph_pixels.clear();
        self.glyph_cursor_x = 0;
        self.glyph_cursor_y = 0;
        self.glyph_row_height = 0;
        self.glyph_used_height = 0;
        self.glyph_locations.clear();
        self.glyph_sources.clear();
        self.clear = clear;
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
            let first = self.shapes.len() as u32;
            for point in mesh.vertices.chunks_exact(2) {
                self.shapes.push(ShapeVertex {
                    position: ndc(point[0], point[1], viewport),
                    local: [0.0; 2],
                    color_a: mesh.rgba,
                    color_b: mesh.rgba,
                    params0: [0.0; 4],
                    params1: [0.0; 4],
                    mode: 0,
                });
            }
            self.batch(
                BatchKind::Shape,
                first,
                self.shapes.len() as u32 - first,
                viewport,
                scissor,
            );
        }
    }

    pub(super) fn box_shadows(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        shadows: &[GpuBoxShadow],
    ) {
        for shadow in shadows {
            let blur = shadow.blur.max(0.5);
            self.shape_quad(
                viewport,
                scissor,
                Rect::new(
                    shadow.x + shadow.offset_x - blur,
                    shadow.y + shadow.offset_y - blur,
                    shadow.w + blur * 2.0,
                    shadow.h + blur * 2.0,
                ),
                shadow.rgba,
                shadow.rgba,
                [
                    shadow.w + blur * 2.0,
                    shadow.h + blur * 2.0,
                    blur,
                    u32::from(shadow.ambient) as f32,
                ],
                shadow.radius.map(|radius| radius + blur),
                5,
                LocalCoordinates::Pixels,
            );
        }
    }

    pub(super) fn glyphs(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        let first = self.glyphs.len() as u32;
        for glyph in glyphs {
            let expected = (glyph.cov_w as usize).saturating_mul(glyph.cov_h as usize);
            if glyph.cov_w == 0 || glyph.cov_h == 0 {
                continue;
            }
            if glyph.coverage.len() < expected
                || glyph.cov_w > GLYPH_ATLAS_WIDTH
                || glyph.cov_h > GLYPH_ATLAS_HEIGHT
            {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    "wgpu glyph coverage does not fit the shared atlas",
                ));
            }
            let key = (glyph.coverage.as_ptr() as usize, glyph.cov_w, glyph.cov_h);
            let (atlas_x, atlas_y) = if let Some(location) = self.glyph_locations.get(&key) {
                *location
            } else {
                if self.glyph_cursor_x + glyph.cov_w > GLYPH_ATLAS_WIDTH {
                    self.glyph_cursor_x = 0;
                    self.glyph_cursor_y = self.glyph_cursor_y.saturating_add(self.glyph_row_height);
                    self.glyph_row_height = 0;
                }
                if self.glyph_cursor_y + glyph.cov_h > GLYPH_ATLAS_HEIGHT {
                    return Err(Error::new(
                        Errc::GraphicsOutOfMemory,
                        "wgpu per-frame glyph atlas is full",
                    ));
                }
                let location = (self.glyph_cursor_x, self.glyph_cursor_y);
                let needed = ((location.1 + glyph.cov_h) * GLYPH_ATLAS_WIDTH) as usize;
                grow_zeroed(&mut self.glyph_pixels, needed);
                for row in 0..glyph.cov_h as usize {
                    let destination = (location.1 as usize + row) * GLYPH_ATLAS_WIDTH as usize
                        + location.0 as usize;
                    let source = row * glyph.cov_w as usize;
                    self.glyph_pixels[destination..destination + glyph.cov_w as usize]
                        .copy_from_slice(&glyph.coverage[source..source + glyph.cov_w as usize]);
                }
                self.glyph_cursor_x = self.glyph_cursor_x.saturating_add(glyph.cov_w + 1);
                self.glyph_row_height = self.glyph_row_height.max(glyph.cov_h + 1);
                self.glyph_used_height = self.glyph_used_height.max(location.1 + glyph.cov_h);
                self.glyph_locations.insert(key, location);
                self.glyph_sources.push(Arc::clone(&glyph.coverage));
                location
            };
            let u0 = atlas_x as f32 / GLYPH_ATLAS_WIDTH as f32;
            let v0 = atlas_y as f32 / GLYPH_ATLAS_HEIGHT as f32;
            let u1 = (atlas_x + glyph.cov_w) as f32 / GLYPH_ATLAS_WIDTH as f32;
            let v1 = (atlas_y + glyph.cov_h) as f32 / GLYPH_ATLAS_HEIGHT as f32;
            let positions = quad_positions(Rect::new(glyph.x, glyph.y, glyph.w, glyph.h));
            let uvs = [[u0, v0], [u1, v0], [u1, v1], [u0, v0], [u1, v1], [u0, v1]];
            for (position, uv) in positions.into_iter().zip(uvs) {
                self.glyphs.push(GlyphVertex {
                    position: ndc(position[0], position[1], viewport),
                    uv,
                    color: glyph.rgba,
                });
            }
        }
        self.batch(
            BatchKind::Glyph,
            first,
            self.glyphs.len() as u32 - first,
            viewport,
            scissor,
        );
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
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let shape_bytes = bytemuck::cast_slice(&self.shapes);
        let glyph_bytes = bytemuck::cast_slice(&self.glyphs);
        let glyph_offset = align_up(shape_bytes.len() as u64, 16);
        let required = glyph_offset.saturating_add(glyph_bytes.len() as u64).max(1);
        if required > self.vertex_capacity {
            self.vertex_capacity = required.next_power_of_two();
            self.vertex_buffer = create_vertex_buffer(device, self.vertex_capacity);
        }
        if !shape_bytes.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, shape_bytes);
        }
        if !glyph_bytes.is_empty() {
            queue.write_buffer(&self.vertex_buffer, glyph_offset, glyph_bytes);
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.glyph_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &self.glyph_pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(GLYPH_ATLAS_WIDTH),
                    rows_per_image: Some(self.glyph_used_height),
                },
                wgpu::Extent3d {
                    width: GLYPH_ATLAS_WIDTH,
                    height: self.glyph_used_height,
                    depth_or_array_layers: 1,
                },
            );
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uix-frame-encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("uix-main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.clear[0] as f64,
                            g: self.clear[1] as f64,
                            b: self.clear[2] as f64,
                            a: self.clear[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut bound = None;
            for batch in &self.batches {
                if bound != Some(batch.kind) {
                    match batch.kind {
                        BatchKind::Shape => {
                            pass.set_pipeline(&self.shape_pipeline);
                            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..glyph_offset));
                        }
                        BatchKind::Glyph => {
                            pass.set_pipeline(&self.glyph_pipeline);
                            pass.set_bind_group(0, &self.glyph_bind_group, &[]);
                            pass.set_vertex_buffer(0, self.vertex_buffer.slice(glyph_offset..));
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
        queue.present(surface_texture);
        Ok(())
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
    ) {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let first = self.shapes.len() as u32;
        let positions = quad_positions(rect);
        let local = coordinates.values(rect);
        for (position, local) in positions.into_iter().zip(local) {
            self.shapes.push(ShapeVertex {
                position: ndc(position[0], position[1], viewport),
                local,
                color_a,
                color_b,
                params0,
                params1,
                mode,
            });
        }
        self.batch(BatchKind::Shape, first, 6, viewport, scissor);
    }

    fn batch(
        &mut self,
        kind: BatchKind,
        first_vertex: u32,
        vertex_count: u32,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
    ) {
        if vertex_count > 0 {
            self.batches.push(DrawBatch {
                kind,
                first_vertex,
                vertex_count,
                viewport,
                scissor,
            });
        }
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

fn ndc(x: f32, y: f32, viewport: (f32, f32)) -> [f32; 2] {
    [
        x / viewport.0.max(1.0) * 2.0 - 1.0,
        1.0 - y / viewport.1.max(1.0) * 2.0,
    ]
}

fn quad_positions(rect: Rect) -> [[f32; 2]; 6] {
    let right = rect.x + rect.w;
    let bottom = rect.y + rect.h;
    [
        [rect.x, rect.y],
        [right, rect.y],
        [right, bottom],
        [rect.x, rect.y],
        [right, bottom],
        [rect.x, bottom],
    ]
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

fn align_up(value: u64, alignment: u64) -> u64 {
    value.saturating_add(alignment - 1) / alignment * alignment
}

pub(crate) fn grow_zeroed(buffer: &mut Vec<u8>, needed: usize) {
    if needed > buffer.len() {
        buffer.resize(needed, 0);
    }
}
