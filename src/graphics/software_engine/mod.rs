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
//! - **AssetStore**: owns immutable-like resources (fonts, images, offscreen
//!   buffers, bitmap font). Lookups by handle pointer comparison.
//! - **RenderTarget**: owns mutable render state (pixels, clip_rect, opacity,
//!   transform, blend_mode). All pixel-level draw operations live here.
//! - **SoftwareEngine**: composes RenderTarget + AssetStore, handles offscreen
//!   target switching via pixel ownership transfer, and implements
//!   GraphicsEngine by delegating to both.
//!
//! The split eliminates 3 `unsafe` pointer casts that previously bypassed the
//! borrow checker — Rust's field-level split borrow allows simultaneous
//! `&self.assets` and `&mut self.rt`.

use crate::graphics::{FontHandle, ImageHandle, Transform};
use crate::graphics::{BlendMode, Rect};

// ════════════════════════════════════════════════════════════════════════════
// 内部数据容器（模块内共享）
// ════════════════════════════════════════════════════════════════════════════

/// Loaded image data (BGRA premultiplied).
pub(crate) struct ImageData {
    pub(crate) pixels: Vec<u32>,
    pub(crate) w: i32,
    pub(crate) h: i32,
}

/// Loaded font data with a rasterized size.
pub(crate) struct FontData {
    pub(crate) font: fontdue::Font,
    pub(crate) size: f32,
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

pub(crate) struct FontSlot {
    pub(crate) handle: Box<FontHandle>,
    pub(crate) data: FontData,
}

pub(crate) struct ImageSlot {
    pub(crate) handle: Box<ImageHandle>,
    pub(crate) data: ImageData,
}

pub(crate) struct OffscreenSlot {
    pub(crate) handle: Box<ImageHandle>,
    pub(crate) data: OffscreenData,
}

// ════════════════════════════════════════════════════════════════════════════
// 子模块
// ════════════════════════════════════════════════════════════════════════════

mod asset_store;
mod core;
mod sdf;
mod shapes;
mod content;
mod engine;
#[cfg(test)]
mod tests;

// ════════════════════════════════════════════════════════════════════════════
// 公开 re-exports
// ════════════════════════════════════════════════════════════════════════════

pub use engine::SoftwareEngine;
