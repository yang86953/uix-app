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

use crate::core::{Point, Result};
use crate::native::traits::platform::Platform;
use crate::native::traits::window::PlatformWindow;
/// Event-driven application window.
///
/// 只负责窗口生命周期和平台句柄管理。
/// 渲染与 present 由 app 主循环和 FrameRenderer 处理。
pub struct Window {
    platform: Box<dyn Platform>,
    window: Option<Box<dyn PlatformWindow>>,
    running: bool,
    exit_code: i32,
    /// 窗口初始尺寸（用于最大化还原时恢复）
    initial_size: (i32, i32),
    /// 调试模式开关（Ctrl+Shift+D 切换；勿用 F12，见 event_loop）。
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
                if let Err(error) = w.center_on_screen() {
                    crate::core::log::warn_fn(format!(
                        "Window::create: center_on_screen failed: {}",
                        error.short_what()
                    ));
                }
                if let Err(error) = w.show() {
                    crate::core::log::error_fn(format!(
                        "Window::create: show failed: {}",
                        error.short_what()
                    ));
                    if let Err(close_error) = w.close() {
                        crate::core::log::warn_fn(format!(
                            "Window::create: cleanup close failed: {}",
                            close_error.short_what()
                        ));
                    }
                    return false;
                }
                if let Err(error) = w.raise() {
                    crate::core::log::warn_fn(format!(
                        "Window::create: raise failed: {}",
                        error.short_what()
                    ));
                }
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

    pub fn show(&mut self) -> Result<()> {
        if let Some(ref mut w) = self.window {
            w.show()?;
        }
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        self.running = false;
        if let Some(ref mut w) = self.window {
            w.close()?;
        }
        Ok(())
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
            let saw_event = Cell::new(false);
            let close_requested = Cell::new(false);
            let collect = |event: &crate::native::traits::event::UiEvent| {
                saw_event.set(true);
                if event.type_ == crate::native::traits::event::UiEventType::WindowClose {
                    close_requested.set(true);
                }
                true
            };

            if !self.platform.event_loop().poll_event(&collect) {
                break;
            }

            if !saw_event.get() && !self.platform.event_loop().wait_event(&collect) {
                break;
            }

            if !saw_event.get() {
                continue;
            }

            if !frame_fn(self.platform.as_mut()) || close_requested.get() {
                self.running = false;
            }
        }
        if let Some(ref mut window) = self.window {
            if let Err(error) = window.close() {
                crate::core::log::warn_fn(format!(
                    "Window::run: close failed: {}",
                    error.short_what()
                ));
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
                if let Err(error) = w.close() {
                    crate::core::log::warn_fn(format!(
                        "Window::drop: close failed: {}",
                        error.short_what()
                    ));
                }
            }
        }
        self.running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::Window;
    use crate::native::test_harness::FakePlatform;
    use crate::native::traits::event::UiEvent;
    use std::cell::Cell;

    #[test]
    fn window_run_does_not_call_frame_fn_without_event() {
        let mut platform = FakePlatform::new();
        platform.event_source.state.exit_after_blocking_calls = Some(1);
        let mut window = Window::new(Box::new(platform));
        let calls = Cell::new(0);

        let status = window.run(|_| {
            calls.set(calls.get() + 1);
            true
        });

        assert_eq!(status, 0);
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn window_run_calls_frame_fn_after_event() {
        let mut platform = FakePlatform::new();
        platform.event_source.inject(UiEvent::close());
        let mut window = Window::new(Box::new(platform));
        let calls = Cell::new(0);

        let status = window.run(|_| {
            calls.set(calls.get() + 1);
            false
        });

        assert_eq!(status, 0);
        assert_eq!(calls.get(), 1);
    }
}
