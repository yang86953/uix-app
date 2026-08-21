// ============================================================================
// platform/linux/wayland/output.rs — wl_output 显示器数据结构
//
// RawOutput 存储通过 wl_output 事件收集的单显示器信息。
// 绑定和收集逻辑已内联到 mod.rs 的 WaylandBackend::new() 中。
// ============================================================================

use crate::core::Rect;
use crate::platform::display::DisplayInfo;

/// 通过 wl_output 事件收集的单显示器信息。
#[derive(Debug, Clone)]
pub(crate) struct RawOutput {
    /// 当前模式分辨率宽度（来自 Mode 事件中 flags 含 WL_OUTPUT_MODE_CURRENT 者）。
    pub(crate) width: i32,
    /// 当前模式分辨率高度。
    pub(crate) height: i32,
    /// compositor 空间中的 x 偏移。
    pub(crate) x: i32,
    /// compositor 空间中的 y 偏移。
    pub(crate) y: i32,
    /// wl_output 缩放因子（整数，如 2 表示 200% HiDPI）。
    pub(crate) scale: i32,
    /// 是否为主显示器（第一个完成 done 的输出视为 primary）。
    pub(crate) is_primary: bool,
}

impl Default for RawOutput {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
            scale: 1,
            is_primary: false,
        }
    }
}

impl RawOutput {
    /// 转换为平台层的 DisplayInfo。
    pub(crate) fn to_display_info(&self) -> DisplayInfo {
        DisplayInfo {
            bounds: Rect::new(
                self.x as f32,
                self.y as f32,
                self.width as f32,
                self.height as f32,
            ),
            dpi_scale: self.scale as f32,
            is_primary: self.is_primary,
        }
    }
}
