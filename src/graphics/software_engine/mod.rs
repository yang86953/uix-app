//! SoftwareEngine — CPU-based 2D software rasterizer.
//!
//! Renders into a BGRA pixel buffer with clipping, blending, gradients,
//! transforms, rounded rectangles, image decoding, font rasterization,
//! and offscreen rendering.
//!
//! Used as fallback when no GPU engine is available.
//!
//! ## Architecture (v2 — split borrow)
//!
//! - **TextBackend**: abstract font loading, layout, and glyph rasterization
//!   (currently `FontdueBackend`, replaceable with `FreeTypeBackend` etc.).
//! - **AssetStore**: owns resource pools (images, offscreen buffers, bitmap
//!   font). Fonts are now managed by the `TextBackend`, not by AssetStore.
//! - **RenderTarget**: owns mutable render state (pixels, clip_rect, opacity,
//!   transform, blend_mode). All pixel-level draw operations live here.
//! - **SoftwareEngine**: composes RenderTarget + AssetStore + TextBackend,
//!   handles offscreen target switching via pixel ownership transfer, and
//!   implements GraphicsEngine by delegating to all three.

use crate::graphics::{BlendMode};
use crate::base::{Rect};
use crate::graphics::{ImageHandle, Transform};

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
mod content;
mod core;
mod engine;
mod fontdue_backend;
mod sdf;
mod shadow;
mod shapes;
#[cfg(test)]
mod tests;

// ════════════════════════════════════════════════════════════════════════════
// 公开 re-exports
// ════════════════════════════════════════════════════════════════════════════

pub use engine::SoftwareEngine;
