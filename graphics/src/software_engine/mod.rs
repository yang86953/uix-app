//! SoftwareEngine — CPU-based 2D software rasterizer (v2).
//!
//! v2 架构: SoftwareEngine 组合 PixelSurface + CpuCanvas2D + AssetStore + FontService。

use uix_core::Rect;
use crate::BlendMode;
use crate::{ImageHandle, Transform};

// ════════════════════════════════════════════════════════════════════════════
// 内部数据容器（模块内共享）
// ════════════════════════════════════════════════════════════════════════════

/// Loaded image data (BGRA premultiplied).
pub(crate) struct ImageData {
    pub(crate) pixels: Vec<u32>,
    pub(crate) w: i32,
    pub(crate) h: i32,
}

/// Offscreen render target.
pub(crate) struct OffscreenData {
    pub(crate) pixels: Vec<u32>,
    pub(crate) w: i32,
    pub(crate) h: i32,
}

/// Snapshot of render state for save/restore.
#[derive(Clone)]
pub(crate) struct RenderState {
    pub(crate) clip_rect: Rect,
    pub(crate) opacity: f32,
    pub(crate) transform: Transform,
    pub(crate) blend_mode: BlendMode,
}

// ════════════════════════════════════════════════════════════════════════════
// Slot types — Boxed handle + data pairs stored in AssetStore
// ════════════════════════════════════════════════════════════════════════════

pub(crate) struct ImageSlot {
    pub(crate) handle: ImageHandle,
    pub(crate) data: ImageData,
}

pub(crate) struct OffscreenSlot {
    pub(crate) handle: ImageHandle,
    pub(crate) data: OffscreenData,
}

// ════════════════════════════════════════════════════════════════════════════
// 子模块
// ════════════════════════════════════════════════════════════════════════════

mod asset_store;
pub(crate) mod ab_glyph_backend;
pub(crate) mod core;
mod engine;
mod font;
mod graphics_impl;

// ════════════════════════════════════════════════════════════════════════════
// 公开 re-exports
// ════════════════════════════════════════════════════════════════════════════

pub use ab_glyph_backend::AbGlyphBackend;
pub use engine::SoftwareEngine;
