// ============================================================================
// platform/types/display.rs — 显示器类型
//
// IDisplay trait 已迁移至 crate::api::traits。
// ============================================================================

use crate::Rect;

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
