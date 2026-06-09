// ============================================================================
// app/window.rs — 基于 Platform 的事件驱动窗口
//
// 核心设计：
//   - 组合 `Box<dyn Platform>`，将窗口管理职责委托给平台层
//   - 不重复维护窗口状态（尺寸、最小化等），全部通过 Platform trait 获取
//   - 提供 run() 事件循环 + 帧回调机制，调用者负责渲染管线
// ============================================================================

use crate::platform::Platform;

/// Event-driven application window.
///
/// Wraps a `dyn Platform` and provides a high-level event loop.
/// Does **not** duplicate platform window state — delegates all window
/// management (dimensions, minimize, maximize, etc.) to the inner `Platform`.
pub struct Window {
    platform: Box<dyn Platform>,
    running: bool,
    exit_code: i32,
}

impl Window {
    /// Create a new Window from a platform implementation.
    pub fn new(platform: Box<dyn Platform>) -> Self {
        Self {
            platform,
            running: false,
            exit_code: 0,
        }
    }

    // ── 平台访问器 ──────────────────────────────────────────────────

    /// Access the underlying platform (read-only).
    pub fn platform(&self) -> &dyn Platform {
        self.platform.as_ref()
    }

    /// Access the underlying platform (mutable).
    pub fn platform_mut(&mut self) -> &mut dyn Platform {
        self.platform.as_mut()
    }

    // ── 窗口生命周期（委托给 platform）────────────────────────────

    /// Create the native window, center it on screen, show it, and raise.
    /// Returns `true` on success.
    pub fn create(&mut self, title: &str, width: i32, height: i32) -> bool {
        if !self.platform.create_window(title, width, height) {
            log::error!("Window::create: platform failed to create window");
            return false;
        }
        // Auto-complete window initialization: center → show → raise
        self.platform.center_on_screen();
        self.platform.show();
        self.platform.raise();
        log::info!(
            "Window created and shown ({}x{}, title='{}')",
            width, height, title
        );
        true
    }

    /// Show the window (called automatically by `create()`).
    pub fn show(&mut self) {
        self.platform.show();
    }

    /// Close the window and signal the event loop to exit.
    pub fn close(&mut self) {
        self.running = false;
    }

    /// Check whether the event loop is still running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    // ── 事件循环 ────────────────────────────────────────────────────

    /// Run the event loop (blocks until exit).
    ///
    /// For each iteration, calls `frame_fn` which receives `&mut dyn Platform`
    /// and returns `true` to continue or `false` to exit the loop.
    ///
    /// The caller is responsible for:
    /// - Polling or waiting for platform events via `platform.poll_event()`
    ///   or `platform.wait_event()`
    /// - Rendering via `GraphicsEngine`
    /// - Presenting the pixel buffer
    ///
    /// No frame rate capping is applied — rendering only happens when
    /// `frame_fn` chooses to do so. Use `wait_event()` inside `frame_fn`
    /// to block until events arrive (0 CPU when idle).
    pub fn run<F>(&mut self, mut frame_fn: F) -> i32
    where
        F: FnMut(&mut dyn Platform) -> bool,
    {
        self.running = true;

        log::info!(
            "Window event loop started ({}x{})",
            self.platform.width(),
            self.platform.height()
        );

        while self.running {
            let should_continue = frame_fn(self.platform.as_mut());
            if !should_continue {
                self.running = false;
            }
        }

        self.running = false;
        log::info!("Window event loop ended");
        self.exit_code
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if self.running {
            // Ensure the native window is destroyed
            self.platform.destroy_window();
        }
        self.running = false;
    }
}
