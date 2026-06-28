// ============================================================================
// platform/types/display.rs — 显示器 API 契约与类型
// ============================================================================

use crate::Rect;

// ════════════════════════════════════════════════════════════════════════════
// IDisplay — 显示器
// ════════════════════════════════════════════════════════════════════════════

pub trait IDisplay {
    fn dpi_scale(&self) -> f32;
    fn is_dark_mode(&self) -> bool;
    fn count(&self) -> i32;
    fn info(&self, index: i32) -> DisplayInfo;
}

// ════════════════════════════════════════════════════════════════════════════
// 显示器信息（display 子系统）
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
