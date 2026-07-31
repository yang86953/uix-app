//! 显示协议 — 显示器信息与 DPI。

use crate::core::error::Result;
use crate::core::geometry::Rect;

// ════════════════════════════════════════════════════════════════════════════
// 显示器信息
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayInfo {
    pub bounds: Rect,
    pub dpi_scale: f32,
    pub is_primary: bool,
}

impl Default for DisplayInfo {
    fn default() -> Self {
        Self {
            bounds: Rect::default(),
            dpi_scale: 1.0,
            is_primary: false,
        }
    }
}

pub trait IDisplay {
    fn dpi_scale(&self) -> Result<f32>;
    fn is_dark_mode(&self) -> Result<bool>;
    fn count(&self) -> Result<i32>;
    fn info(&self, index: i32) -> Result<DisplayInfo>;
}
