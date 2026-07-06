// ============================================================================
// app/window.rs — 基于 Platform 的事件驱动窗口
//
// 职责边界（app 层只负责窗口生命周期管理）：
//   - 创建/销毁平台窗口
//   - 窗口属性（标题、尺寸、显隐）
//   - 调试模式开关
//   - 提供 platform / platform_window 访问
//
// 超出边界的（应在 ui 层）：
//   - RenderPipeline 帧渲染编排
//   - 渲染循环（begin_frame / end_frame / present）
//   - Widget 树渲染、LayerTree 管理
// ============================================================================

use std::cell::Cell;

use crate::core::Point;
use crate::native::traits::platform::Platform;
use crate::native::traits::window::PlatformWindow;
/// Event-driven application window.
///
/// 只负责窗口生命周期和平台句柄管理。
/// 渲染循环由 ui::render_loop 处理。
pub struct Window {
    platform: Box<dyn Platform>,
    window: Option<Box<dyn PlatformWindow>>,
    running: bool,
    exit_code: i32,
    /// 窗口初始尺寸（用于最大化还原时恢复）
    initial_size: (i32, i32),
    /// 调试模式开关（F12 切换）。
    debug_mode: Cell<bool>,
    /// 当前光标位置（用于调试模式悬浮高亮）。
    cursor_pos: Cell<Point>,
}

impl Window {
    pub fn new(platform: Box<dyn Platform>) -> Self {
        let debug_mode = std::env::var("UIX_DEBUG").is_ok();
        if debug_mode {
            crate::core::log::info_fn("[Debug] UIX_DEBUG 环境变量已设置，调试模式默认开启");
        }
        Self {
            platform,
            window: None,
            running: false,
            exit_code: 0,
            initial_size: (800, 600),
            debug_mode: Cell::new(debug_mode),
            cursor_pos: Cell::new(Point::new(0.0, 0.0)),
        }
    }

    pub fn platform(&self) -> &dyn Platform {
        self.platform.as_ref()
    }

    pub fn platform_mut(&mut self) -> &mut dyn Platform {
        self.platform.as_mut()
    }

    /// 获取当前窗口句柄。
    pub fn window_box(&self) -> &Option<Box<dyn PlatformWindow>> {
        &self.window
    }

    /// 获取当前窗口大小。
    pub fn size(&self) -> (i32, i32) {
        self.initial_size
    }

    /// 创建平台窗口，存储 PlatformWindow 句柄。
    pub fn create(&mut self, title: &str, width: i32, height: i32) -> bool {
        self.initial_size = (width, height);
        match self
            .platform
            .window_manager()
            .create_window(title, width, height)
        {
            Ok(mut w) => {
                w.center_on_screen();
                w.show();
                w.raise();
                crate::core::log::info_fn(format!(
                    "Window created and shown ({}x{}, title='{}')",
                    width, height, title
                ));
                self.window = Some(w);
                true
            }
            Err(e) => {
                crate::core::log::error_fn(format!(
                    "Window::create: platform failed: {}",
                    e.short_what()
                ));
                false
            }
        }
    }

    pub fn show(&mut self) {
        if let Some(ref mut w) = self.window {
            w.show();
        }
    }

    pub fn close(&mut self) {
        self.running = false;
        if let Some(ref mut w) = self.window {
            w.close();
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// 更新光标位置（由平台层事件设置）。
    pub fn set_cursor_pos(&self, pos: Point) {
        self.cursor_pos.set(pos);
    }

    /// 获取当前光标位置。
    pub fn cursor_pos(&self) -> Point {
        self.cursor_pos.get()
    }

    /// 设置调试模式。
    pub fn set_debug_mode(&self, mode: bool) {
        self.debug_mode.set(mode);
        crate::core::log::info_fn(format!(
            "[Debug] 调试模式 {}",
            if mode { "开启" } else { "关闭" }
        ));
    }

    /// 查询当前是否处于调试模式。
    pub fn debug_mode(&self) -> bool {
        self.debug_mode.get()
    }

    /// 简单事件循环（无渲染），由上层自行驱动。
    pub fn run<F>(&mut self, mut frame_fn: F) -> i32
    where
        F: FnMut(&mut dyn Platform) -> bool,
    {
        self.running = true;
        if let Some(ref w) = self.window {
            crate::core::log::info_fn(format!(
                "Window event loop started ({}x{})",
                w.properties().width(),
                w.properties().height()
            ));
        }
        while self.running {
            if !frame_fn(self.platform.as_mut()) {
                self.running = false;
            }
        }
        self.running = false;
        crate::core::log::info_fn("Window event loop ended");
        self.exit_code
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if self.running {
            if let Some(ref mut w) = self.window {
                w.close();
            }
        }
        self.running = false;
    }
}
