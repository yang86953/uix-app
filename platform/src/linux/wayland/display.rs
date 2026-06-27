// ============================================================================
// platform/linux/wayland/display.rs — IDisplay impl for WaylandBackend
//
// 使用 wl_output 协议收集的显示器几何和缩放因子，提供真实查询。
// 回退到硬编码值（1920×1080, scale=1）当 wl_output 未绑定或零输出时。
// ============================================================================

use crate::types::DisplayInfo;
use crate::IDisplay;

use super::WaylandBackend;

impl IDisplay for WaylandBackend {
    fn dpi_scale(&self) -> f32 {
        self.outputs
            .lock()
            .ok()
            .and_then(|list| {
                list.iter()
                    .find(|o| o.is_primary)
                    .or_else(|| list.first())
                    .map(|o| o.scale as f32)
            })
            .unwrap_or(1.0)
    }

    fn is_dark_mode(&self) -> bool {
        std::env::var("GTK_THEME")
            .map(|t| t.contains("dark"))
            .unwrap_or(false)
    }

    fn count(&self) -> i32 {
        self.outputs
            .lock()
            .map(|list| list.len() as i32)
            .unwrap_or(1)
    }

    fn info(&self, index: i32) -> DisplayInfo {
        let idx = index.max(0) as usize;
        self.outputs
            .lock()
            .ok()
            .and_then(|list| list.get(idx).map(|o| o.to_display_info()))
            .unwrap_or_else(|| {
                // 回退：虚拟 1080p 显示器
                DisplayInfo {
                    bounds: crate::Rect::new(0.0, 0.0, 1920.0, 1080.0),
                    dpi_scale: 1.0,
                    is_primary: index == 0,
                }
            })
    }
}
