// ============================================================================
// platform/shared/state.rs — 统一窗口状态（跨平台共享）
//
// 所有平台的窗口状态变量统一在此定义，消除各平台各自维护状态的重复。
// ============================================================================

use crate::core::{Point, WindowId};

/// 窗口状态（纯数据，无逻辑）
#[derive(Debug, Clone)]
pub(crate) struct WindowState {
    pub(crate) window_id: WindowId,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) minimum_size: Option<(i32, i32)>,
    pub(crate) maximum_size: Option<(i32, i32)>,
    pub(crate) pos_x: i32,
    pub(crate) pos_y: i32,
    pub(crate) visible: bool,
    pub(crate) resizable: bool,
    pub(crate) borderless: bool,
    pub(crate) fullscreen: bool,
    pub(crate) maximized: bool,
    pub(crate) minimized: bool,
    pub(crate) always_on_top: bool,
    pub(crate) opacity: f32,
    pub(crate) file_drop_enabled: bool,
    pub(crate) text_input_active: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            window_id: WindowId::ROOT,
            width: 800,
            height: 600,
            minimum_size: None,
            maximum_size: None,
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
    // 保留无 ID 的共享状态构造器，供平台测试和外部组装使用。
    #[allow(dead_code)]
    pub(crate) fn with_size(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            ..Default::default()
        }
    }

    /// 创建指定 ID 与尺寸的初始窗口状态。
    // 保留带窗口身份的共享状态构造器，供平台测试和外部组装使用。
    #[allow(dead_code)]
    pub(crate) fn with_id_and_size(window_id: WindowId, width: i32, height: i32) -> Self {
        Self {
            window_id,
            width,
            height,
            ..Default::default()
        }
    }

    /// 返回窗口位置。
    // 保留位置快照访问器，供平台状态同步方按需读取。
    #[allow(dead_code)]
    pub(crate) fn position(&self) -> Point {
        Point::new(self.pos_x as f32, self.pos_y as f32)
    }
}
