//! 共享 wgpu 绘制流：顶点布局、批类型与 per-target 命令缓冲。

use std::{collections::HashMap, sync::Arc};

use bytemuck::{Pod, Zeroable};

use crate::core::Rect;
use crate::native::graphics::wgpu_backend::glyph_cover::GlyphCoverDraw;

/// 每帧字形 atlas 边长（R8 coverage 与 RGBA8 MSDF 共用尺寸）。
pub(super) const GLYPH_ATLAS_WIDTH: u32 = 1024;
pub(super) const GLYPH_ATLAS_HEIGHT: u32 = 1024;
pub(super) const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct ShapeVertex {
    pub position: [f32; 2],
    pub local: [f32; 2],
    pub color_a: [f32; 4],
    pub color_b: [f32; 4],
    pub params0: [f32; 4],
    pub params1: [f32; 4],
    pub mode: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct GlyphVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct TextureVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub opacity: f32,
    pub _pad: [f32; 3],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum BatchKind {
    Shape,
    /// 与 Shape 共用顶点布局，但使用 replace blend（脏区透明清除）。
    Clear,
    /// CPU coverage → R8 atlas 采样。
    Glyph,
    /// 轮廓边 → RGBA8 MSDF atlas，median 采样。
    GlyphMsdf,
    Texture {
        bind_group: u32,
        additive: bool,
    },
}

pub(super) struct DrawBatch {
    pub kind: BatchKind,
    pub first_vertex: u32,
    pub vertex_count: u32,
    pub viewport: (f32, f32),
    pub scissor: Option<(i32, i32, i32, i32)>,
}

/// Swapchain 与 offscreen 各自独立命令流，避免 Picture 光栅冲掉主帧几何。
pub(super) struct DrawStream {
    pub shapes: Vec<ShapeVertex>,
    pub glyphs: Vec<GlyphVertex>,
    /// MSDF 字形与 R8 coverage 共用顶点布局，分批绑定不同 atlas。
    pub msdf_glyphs: Vec<GlyphVertex>,
    pub textures: Vec<TextureVertex>,
    pub batches: Vec<DrawBatch>,
    pub glyph_pixels: Vec<u8>,
    pub glyph_cursor_x: u32,
    pub glyph_cursor_y: u32,
    pub glyph_row_height: u32,
    pub glyph_used_height: u32,
    pub glyph_locations: HashMap<(usize, u32, u32), (u32, u32)>,
    pub glyph_sources: Vec<Arc<[u8]>>,
    pub msdf_cursor_x: u32,
    pub msdf_cursor_y: u32,
    pub msdf_row_height: u32,
    pub msdf_used_height: u32,
    pub msdf_locations: HashMap<(usize, u32, u32), (u32, u32)>,
    pub glyph_mesh_sources: Vec<Arc<[f32]>>,
    pub glyph_covers: Vec<GlyphCoverDraw>,
    pub clear: [f32; 4],
    pub needs_clear: bool,
}

impl DrawStream {
    pub(super) fn new() -> Self {
        Self {
            shapes: Vec::new(),
            glyphs: Vec::new(),
            msdf_glyphs: Vec::new(),
            textures: Vec::new(),
            batches: Vec::new(),
            glyph_pixels: Vec::new(),
            glyph_cursor_x: 0,
            glyph_cursor_y: 0,
            glyph_row_height: 0,
            glyph_used_height: 0,
            glyph_locations: HashMap::new(),
            glyph_sources: Vec::new(),
            msdf_cursor_x: 0,
            msdf_cursor_y: 0,
            msdf_row_height: 0,
            msdf_used_height: 0,
            msdf_locations: HashMap::new(),
            glyph_mesh_sources: Vec::new(),
            glyph_covers: Vec::new(),
            clear: [0.0; 4],
            needs_clear: true,
        }
    }

    pub(super) fn reset(&mut self, clear: [f32; 4]) {
        self.shapes.clear();
        self.glyphs.clear();
        self.msdf_glyphs.clear();
        self.textures.clear();
        self.batches.clear();
        self.glyph_pixels.clear();
        self.glyph_cursor_x = 0;
        self.glyph_cursor_y = 0;
        self.glyph_row_height = 0;
        self.glyph_used_height = 0;
        self.glyph_locations.clear();
        self.glyph_sources.clear();
        self.msdf_cursor_x = 0;
        self.msdf_cursor_y = 0;
        self.msdf_row_height = 0;
        self.msdf_used_height = 0;
        self.msdf_locations.clear();
        self.glyph_mesh_sources.clear();
        self.glyph_covers.clear();
        self.clear = clear;
        self.needs_clear = true;
    }

    pub(super) fn clear_commands_only(&mut self) {
        self.shapes.clear();
        self.glyphs.clear();
        self.msdf_glyphs.clear();
        self.textures.clear();
        self.batches.clear();
        self.glyph_pixels.clear();
        self.glyph_cursor_x = 0;
        self.glyph_cursor_y = 0;
        self.glyph_row_height = 0;
        self.glyph_used_height = 0;
        self.glyph_locations.clear();
        self.glyph_sources.clear();
        self.msdf_cursor_x = 0;
        self.msdf_cursor_y = 0;
        self.msdf_row_height = 0;
        self.msdf_used_height = 0;
        self.msdf_locations.clear();
        self.glyph_mesh_sources.clear();
        self.glyph_covers.clear();
        self.needs_clear = false;
    }

    /// 保留缓冲已由外部写入有效像素（如 overlay backdrop restore），后续 flush 用 Load。
    pub(super) fn mark_content_retained(&mut self) {
        self.needs_clear = false;
    }

    pub(super) fn is_empty(&self) -> bool {
        self.batches.is_empty() && !self.needs_clear
    }

    pub(super) fn push_batch(
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ActiveTarget {
    Swapchain,
    Offscreen,
}

pub(super) fn ndc(x: f32, y: f32, viewport: (f32, f32)) -> [f32; 2] {
    [
        x / viewport.0.max(1.0) * 2.0 - 1.0,
        1.0 - y / viewport.1.max(1.0) * 2.0,
    ]
}

pub(super) fn quad_positions(rect: Rect) -> [[f32; 2]; 6] {
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

pub(super) fn align_up(value: u64, alignment: u64) -> u64 {
    value.saturating_add(alignment - 1) / alignment * alignment
}

pub(super) fn align_up_u32(value: u32, alignment: u32) -> u32 {
    value.saturating_add(alignment - 1) / alignment * alignment
}

pub(crate) fn grow_zeroed(buffer: &mut Vec<u8>, needed: usize) {
    if needed > buffer.len() {
        buffer.resize(needed, 0);
    }
}
