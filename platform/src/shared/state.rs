// ============================================================================
// platform/shared/state.rs — 统一窗口状态（跨平台共享）
//
// 所有平台的窗口状态变量统一在此定义，消除各平台各自维护状态的重复。
// ============================================================================

use crate::Point;

/// 窗口状态（纯数据，无逻辑）
#[derive(Debug, Clone)]
pub struct WindowState {
    pub width: i32,
    pub height: i32,
    pub pos_x: i32,
    pub pos_y: i32,
    pub visible: bool,
    pub resizable: bool,
    pub borderless: bool,
    pub fullscreen: bool,
    pub maximized: bool,
    pub minimized: bool,
    pub always_on_top: bool,
    pub opacity: f32,
    pub file_drop_enabled: bool,
    pub text_input_active: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            pos_x: 0,
            pos_y: 0,
            visible: false,
            resizable: true,
            borderless: false,
            fullscreen: false,
            maximized: false,
            minimized: false,
            always_on_top: false,
            opacity: 1.0,
            file_drop_enabled: false,
            text_input_active: false,
        }
    }
}

impl WindowState {
    /// 创建指定尺寸的初始窗口状态。
    pub fn with_size(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            ..Default::default()
        }
    }

    /// 返回窗口位置。
    pub fn position(&self) -> Point {
        Point::new(self.pos_x as f32, self.pos_y as f32)
    }
}
