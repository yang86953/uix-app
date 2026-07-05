//! 渲染后端协议 — 像素缓冲表面。

use super::primitives::Color;
use crate::platform::{Rect, Size};

pub trait RenderingBackend {
    fn surface_size(&self) -> Size;
    fn pixels(&self) -> &[u32];
    fn pixels_mut(&mut self) -> &mut [u32];
    fn clear_rect(&mut self, rect: Rect, color: Color);
    fn present(&mut self);
    fn copy_region(&mut self, _src: Rect, _dst_x: i32, _dst_y: i32) {}
}

pub use crate::render::backend::{BackendCapabilities, BackendKind, DamageRegion, RenderBackend};
