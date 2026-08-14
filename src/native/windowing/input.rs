//! 输入协议 — 键盘、鼠标、光标、剪贴板与 IME。

use crate::core::error::Result;
use crate::core::geometry::{Point, Rect};

// 输入值类型与剪贴板契约已收口到 platform 公开面；此处保持既有
// `crate::native::windowing::input::*` 路径可解析（类型所有权归 platform）。
pub use crate::platform::windowing::{
    ControlSize, CursorType, IClipboard, KeyCode, KeyMod, MouseButton, ScrollDirection,
};

pub trait ICursor {
    fn set_cursor(&mut self, cursor: CursorType) -> Result<()>;
    fn show_cursor(&mut self, visible: bool) -> Result<()>;
    fn cursor_position(&self) -> Result<Point>;
    fn set_cursor_position(&mut self, x: i32, y: i32) -> Result<()>;
    fn confine_cursor(&mut self, confine: bool) -> Result<()>;
    fn capture_mouse(&mut self) -> Result<()>;
    fn release_mouse(&mut self) -> Result<()>;
}

pub trait ITextInput {
    /// Select the native window that owns the following IME session calls.
    ///
    /// Text input is window-scoped even when a platform exposes one process-wide
    /// IME service. Implementations must switch both the native target and the
    /// `WindowId` used to tag emitted events as one operation.
    fn set_target_window(
        &mut self,
        window_id: crate::core::WindowId,
        native_window: *mut std::ffi::c_void,
    ) -> Result<()>;
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn set_cursor_rect(&mut self, rect: Rect) -> Result<()>;
}

pub trait IKeyboard {
    fn is_down(&self, key: KeyCode) -> bool;
    fn idle_ms(&self) -> u32;
    fn double_click_ms(&self) -> u32;
}
