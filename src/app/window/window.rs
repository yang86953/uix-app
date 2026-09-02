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

// 独立 Window 入口复用统一自动居中结果分类。
use crate::app::window::window_actions::report_center_on_screen_result;
// 独立 Window 入口复用 Application 的能力创建策略。
use crate::app::window::window_creation::create_app_window;
use crate::core::{Point, Result};
use crate::diagnostics::Diagnostics;
use crate::platform::platform::PlatformSystem;
use crate::platform::windowing::WindowSurfaceRole;
use crate::platform::windowing::window::PlatformWindow;
/// Event-driven application window.
///
/// 只负责窗口生命周期和平台句柄管理。
/// 渲染与 present 由 app 主循环和 ScenePipeline 处理。
pub struct Window {
    platform: Box<dyn PlatformSystem>,
    window: Option<Box<dyn PlatformWindow>>,
    running: bool,
    exit_code: i32,
    /// 窗口初始尺寸（用于最大化还原时恢复）
    initial_size: (i32, i32),
    /// 独立窗口拥有的 Diagnostics System；调试模式由其统一保存。
    diagnostics: Diagnostics,
    /// 当前光标位置（用于调试模式悬浮高亮）。
    cursor_pos: Cell<Point>,
}

impl Window {
    /// 创建拥有指定平台实现、尚未建立平台窗口的应用窗口控制器。
    pub fn new(platform: Box<dyn PlatformSystem>) -> Self {
        let diagnostics = Diagnostics::default();
        match crate::diagnostics::debug_mode_from_env() {
            Some(Ok(enabled)) => diagnostics.set_debug_mode(enabled),
            Some(Err(())) => tracing::warn!(
                target: "uix::diagnostics",
                debug_event = "invalid_debug_switch",
                "UIX_DEBUG must be one of 1/0, true/false, yes/no, or on/off"
            ),
            None => {}
        }
        Self {
            platform,
            window: None,
            running: false,
            exit_code: 0,
            initial_size: (800, 600),
            diagnostics,
            cursor_pos: Cell::new(Point::new(0.0, 0.0)),
        }
    }

    /// 返回窗口控制器拥有的平台实现共享借用。
    pub fn platform(&self) -> &dyn PlatformSystem {
        self.platform.as_ref()
    }

    /// 返回窗口控制器拥有的平台实现可变借用。
    pub fn platform_mut(&mut self) -> &mut dyn PlatformSystem {
        self.platform.as_mut()
    }

    /// 获取当前窗口句柄。
    pub fn window_box(&self) -> &Option<Box<dyn PlatformWindow>> {
        &self.window
    }

    /// 返回创建窗口时记录的初始尺寸。
    pub fn size(&self) -> (i32, i32) {
        self.initial_size
    }

    /// 创建平台窗口，存储 PlatformWindow 句柄。
    pub fn create(&mut self, title: &str, width: i32, height: i32) -> bool {
        self.initial_size = (width, height);
        match create_app_window(
            self.platform.window_manager(),
            title,
            width,
            height,
            &WindowSurfaceRole::Toplevel,
        ) {
            Ok(mut w) => {
                // Wayland layer-shell 可在首个 configure 中替换零维请求。
                self.initial_size = w.client_logical_extent();
                // 独立 Window 入口与 App 主窗、次窗保持相同能力缺失语义。
                report_center_on_screen_result(
                    &self.diagnostics,
                    // 保留可定位的调用入口上下文。
                    "Window::create: center_on_screen failed",
                    // 平台 typed result 直接交给统一分类器。
                    w.center_on_screen(),
                    // 结束独立窗口自动居中报告。
                );
                if let Err(error) = w.show() {
                    tracing::error!("Window::create: show failed: {}", error.short_what());
                    // show 失败导致创建终态失败：主错误与清理失败都进框架报告。
                    self.diagnostics.report_with_origin(
                        error,
                        crate::diagnostics::ReportOrigin::framework("window", "show"),
                    );
                    if let Err(close_error) = w.close() {
                        tracing::warn!(
                            "Window::create: cleanup close failed: {}",
                            close_error.short_what()
                        );
                        self.diagnostics.report_with_origin(
                            close_error,
                            crate::diagnostics::ReportOrigin::framework(
                                "window",
                                "create_cleanup_close",
                            ),
                        );
                    }
                    return false;
                }
                if let Err(error) = w.raise() {
                    tracing::warn!("Window::create: raise failed: {}", error.short_what());
                    self.diagnostics.report_with_origin(
                        error,
                        crate::diagnostics::ReportOrigin::framework("window", "raise"),
                    );
                }
                tracing::info!(
                    "Window created and shown ({}x{}, title='{}')",
                    width,
                    height,
                    title
                );
                self.window = Some(w);
                true
            }
            Err(e) => {
                tracing::error!("Window::create: platform failed: {}", e.short_what());
                // 独立 Window 入口的创建终态失败同样进入框架报告。
                self.diagnostics.report_with_origin(
                    e,
                    crate::diagnostics::ReportOrigin::framework("window", "create"),
                );
                false
            }
        }
    }

    /// 显示已创建的平台窗口；尚未创建窗口时直接成功。
    pub fn show(&mut self) -> Result<()> {
        if let Some(ref mut w) = self.window {
            w.show()?;
        }
        Ok(())
    }

    /// 停止事件循环并关闭已创建的平台窗口。
    pub fn close(&mut self) -> Result<()> {
        self.running = false;
        if let Some(ref mut w) = self.window {
            w.close()?;
        }
        Ok(())
    }

    /// 返回简易事件循环当前是否正在运行。
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
        self.diagnostics.set_debug_mode(mode);
    }

    /// 查询当前是否处于调试模式。
    pub fn debug_mode(&self) -> bool {
        self.diagnostics.debug_mode()
    }

    /// 简单事件循环（无渲染），由上层自行驱动。
    pub fn run<F>(&mut self, mut frame_fn: F) -> i32
    where
        F: FnMut(&mut dyn PlatformSystem) -> bool,
    {
        self.running = true;
        if let Some(ref w) = self.window {
            tracing::info!(
                "Window event loop started ({}x{})",
                w.properties().width(),
                w.properties().height()
            );
        }
        while self.running {
            let saw_event = Cell::new(false);
            let close_requested = Cell::new(false);
            let collect = |event: &crate::platform::windowing::event::UiEvent| {
                saw_event.set(true);
                if event.type_ == crate::platform::windowing::event::UiEventType::WindowClose {
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
                tracing::warn!("Window::run: close failed: {}", error.short_what());
                self.diagnostics.report_with_origin(
                    error,
                    crate::diagnostics::ReportOrigin::framework("window", "close"),
                );
            }
        }
        self.running = false;
        tracing::info!("Window event loop ended");
        self.exit_code
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if self.running {
            if let Some(ref mut w) = self.window {
                if let Err(error) = w.close() {
                    tracing::warn!("Window::drop: close failed: {}", error.short_what());
                    self.diagnostics.report_with_origin(
                        error,
                        crate::diagnostics::ReportOrigin::framework("window", "close"),
                    );
                }
            }
        }
        self.running = false;
    }
}
