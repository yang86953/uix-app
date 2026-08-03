//! One GPU renderer shared by every wgpu backend.

use std::mem::size_of;

use crate::core::{Errc, Error, Rect, Result};
use crate::native::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuImageBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSector,
    GpuSolidMesh, GpuSolidRect, GpuStrokeRect,
};
use crate::native::presentation::graphics::wgpu_backend::draw_stream::{
    align_up, align_up_u32, ndc, quad_positions, BatchKind, DrawStream, GlyphVertex, ShapeVertex,
    TextureVertex, COPY_BYTES_PER_ROW_ALIGNMENT, GLYPH_ATLAS_HEIGHT, GLYPH_ATLAS_WIDTH,
};
use crate::native::presentation::graphics::wgpu_backend::glyph_batch;
use crate::native::presentation::graphics::wgpu_backend::glyph_cover::GlyphCoverPipeline;

pub(super) use crate::native::presentation::graphics::wgpu_backend::draw_stream::ActiveTarget;

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

pub(super) struct WgpuExecutor {
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

mod exec;
mod exec2;
mod exec3;
mod helpers;

use self::helpers::*;
