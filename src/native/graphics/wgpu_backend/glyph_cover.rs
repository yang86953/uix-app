//! 将字形轮廓边写入共享 RGBA8 MSDF atlas（多通道距离场）。

use std::mem::size_of;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

use crate::core::{Errc, Error, Result};
use crate::draw::font::glyph_outline::{colorize_edges, is_outline_edges};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CoverVertex {
    /// Atlas 归一化 clip 坐标。
    position: [f32; 2],
    /// 槽位左上角（atlas 像素）。
    atlas_origin: [f32; 2],
    edge_start: u32,
    edge_count: u32,
}

/// 一帧内待写入 MSDF atlas 的轮廓覆盖绘制（边列表）。
pub(super) struct GlyphCoverDraw {
    pub atlas_x: u32,
    pub atlas_y: u32,
    pub width: u32,
    pub height: u32,
    /// `[ax,ay,bx,by,…]` 本地像素边。
    pub mesh: Arc<[f32]>,
}

pub(super) struct GlyphCoverPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    edge_buffer: wgpu::Buffer,
    edge_capacity: u64,
    color_buffer: wgpu::Buffer,
    color_capacity: u64,
}

impl GlyphCoverPipeline {
    pub(super) fn new(device: &wgpu::Device) -> Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uix-glyph-cover-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("glyph_cover.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uix-glyph-cover-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("uix-glyph-cover-layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uix-glyph-cover-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("cover_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<CoverVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Uint32,
                        },
                        wgpu::VertexAttribute {
                            offset: 20,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Uint32,
                        },
                    ],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("cover_fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    // 每个槽位一张全覆盖四边形，直接覆写 MSDF。
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let vertex_capacity = 4096u64;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uix-glyph-cover-vertices"),
            size: vertex_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let edge_capacity = 65536u64;
        let edge_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uix-glyph-cover-edges"),
            size: edge_capacity,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let color_capacity = 16384u64;
        let color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uix-glyph-cover-colors"),
            size: color_capacity,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            pipeline,
            bind_group_layout,
            vertex_buffer,
            vertex_capacity,
            edge_buffer,
            edge_capacity,
            color_buffer,
            color_capacity,
        })
    }

    /// 将轮廓边栅格进 RGBA8 MSDF atlas；调用方须保证 atlas 当前不在采样中。
    pub(super) fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        atlas_view: &wgpu::TextureView,
        atlas_width: u32,
        atlas_height: u32,
        draws: &[GlyphCoverDraw],
    ) -> Result<()> {
        if draws.is_empty() {
            return Ok(());
        }
        let mut edge_vecs: Vec<[f32; 4]> = Vec::new();
        let mut edge_colors: Vec<u32> = Vec::new();
        let mut vertices = Vec::<CoverVertex>::new();
        // 与 vertices 同步：仅包含实际写入的字形，避免退化边被过滤后错位。
        let mut active: Vec<(u32, u32, u32, u32)> = Vec::new();
        let atlas_w = atlas_width.max(1) as f32;
        let atlas_h = atlas_height.max(1) as f32;
        let to_clip = |x: f32, y: f32| -> [f32; 2] {
            [x / atlas_w * 2.0 - 1.0, 1.0 - y / atlas_h * 2.0]
        };

        for draw in draws {
            if draw.width == 0
                || draw.height == 0
                || !is_outline_edges(draw.mesh.as_ref())
            {
                continue;
            }
            if draw.atlas_x.saturating_add(draw.width) > atlas_width
                || draw.atlas_y.saturating_add(draw.height) > atlas_height
            {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    "wgpu glyph cover draw exceeds atlas",
                ));
            }
            let Some(colors) = colorize_edges(draw.mesh.as_ref()) else {
                continue;
            };
            let edge_start = edge_vecs.len() as u32;
            let mut i = 0usize;
            let mut edge_idx = 0usize;
            while i + 3 < draw.mesh.len() {
                let ax = draw.mesh[i];
                let ay = draw.mesh[i + 1];
                let bx = draw.mesh[i + 2];
                let by = draw.mesh[i + 3];
                i += 4;
                let mask = colors.get(edge_idx).copied().unwrap_or(crate::draw::font::glyph_outline::EDGE_WHITE);
                edge_idx += 1;
                if (ax - bx).abs() <= 1e-6 && (ay - by).abs() <= 1e-6 {
                    continue;
                }
                if mask == 0 {
                    continue;
                }
                edge_vecs.push([ax, ay, bx, by]);
                edge_colors.push(u32::from(mask));
            }
            let edge_count = (edge_vecs.len() as u32).saturating_sub(edge_start);
            if edge_count == 0 {
                continue;
            }
            let ax = draw.atlas_x as f32;
            let ay = draw.atlas_y as f32;
            let bx = ax + draw.width as f32;
            let by = ay + draw.height as f32;
            let origin = [ax, ay];
            // 两个三角形覆盖槽位。
            let corners = [
                (ax, ay),
                (bx, ay),
                (bx, by),
                (ax, ay),
                (bx, by),
                (ax, by),
            ];
            for (x, y) in corners {
                vertices.push(CoverVertex {
                    position: to_clip(x, y),
                    atlas_origin: origin,
                    edge_start,
                    edge_count,
                });
            }
            active.push((draw.atlas_x, draw.atlas_y, draw.width, draw.height));
        }
        if vertices.is_empty() || edge_vecs.is_empty() || active.is_empty() {
            return Ok(());
        }

        let edge_bytes = bytemuck::cast_slice(&edge_vecs);
        let edge_needed = edge_bytes.len() as u64;
        if edge_needed > self.edge_capacity {
            self.edge_capacity = edge_needed.next_power_of_two().max(65536);
            self.edge_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("uix-glyph-cover-edges"),
                size: self.edge_capacity,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.edge_buffer, 0, edge_bytes);

        let color_bytes = bytemuck::cast_slice(&edge_colors);
        let color_needed = color_bytes.len() as u64;
        if color_needed > self.color_capacity {
            self.color_capacity = color_needed.next_power_of_two().max(16384);
            self.color_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("uix-glyph-cover-colors"),
                size: self.color_capacity,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.color_buffer, 0, color_bytes);

        let vert_bytes = bytemuck::cast_slice(&vertices);
        let vert_needed = vert_bytes.len() as u64;
        if vert_needed > self.vertex_capacity {
            self.vertex_capacity = vert_needed.next_power_of_two().max(4096);
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("uix-glyph-cover-vertices"),
                size: self.vertex_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.vertex_buffer, 0, vert_bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uix-glyph-cover-bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.edge_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.color_buffer.as_entire_binding(),
                },
            ],
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("uix-glyph-cover-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: atlas_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, Some(&bind_group), &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            // 每个字形 6 顶点；按字形设 scissor 避免写穿邻槽。
            for (index, &(ax, ay, w, h)) in active.iter().enumerate() {
                let vertex_base = (index as u32).saturating_mul(6);
                pass.set_scissor_rect(ax, ay, w, h);
                pass.draw(vertex_base..vertex_base + 6, 0..1);
            }
        }
        Ok(())
    }
}
