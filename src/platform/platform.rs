// ============================================================================
// platform/platform.rs — 平台抽象接口（窗口管理 · 事件循环 · 子系统访问器）
//
// 核心职责：窗口管理 · 事件循环
// 扩展能力：通过 accessor 访问各子系统接口
// ============================================================================

use crate::platform::event::*;
use crate::platform::types::*;
use crate::platform::*;

// ════════════════════════════════════════════════════════════════════════════
// Platform — 平台抽象接口
// ════════════════════════════════════════════════════════════════════════════

pub trait Platform {
    // ── 窗口管理（核心职责）────────────────────────────────────────

    fn create_window(&mut self, title: &str, width: i32, height: i32) -> bool;
    fn destroy_window(&mut self);
    fn native_window(&self) -> *mut std::ffi::c_void;
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    fn set_title(&mut self, title: &str);
    fn set_resizable(&mut self, resizable: bool);
    fn maximize(&mut self);
    fn minimize(&mut self);
    fn restore(&mut self);
    fn is_maximized(&self) -> bool;
    fn is_minimized(&self) -> bool;
    fn set_position(&mut self, x: i32, y: i32);
    fn position(&self) -> Point;
    fn set_size(&mut self, w: i32, h: i32);
    fn set_minimum_size(&mut self, w: i32, h: i32);
    fn set_maximum_size(&mut self, w: i32, h: i32);
    fn center_on_screen(&mut self);
    fn show(&mut self);
    fn hide(&mut self);
    fn is_visible(&self) -> bool;
    fn set_always_on_top(&mut self, on: bool);
    fn set_borderless(&mut self, borderless: bool);
    fn set_fullscreen(&mut self, fullscreen: bool);
    fn is_fullscreen(&self) -> bool;
    fn set_window_opacity(&mut self, opacity: f32);
    fn flash_window(&mut self);
    fn raise(&mut self);
    fn lower(&mut self);
    fn set_window_icon(&mut self, icon_path: &str);

    // ── 事件循环（核心职责）────────────────────────────────────────

    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;

    // ── 输入控制 ────────────────────────────────────────────────────

    fn start_text_input(&mut self);
    fn stop_text_input(&mut self);

    // ── 文件拖放 ────────────────────────────────────────────────────

    fn enable_file_drop(&mut self, enable: bool);

    // ── 子系统访问器（组合模式）────────────────────────────────────

    fn clipboard(&mut self) -> &mut dyn IClipboard;
    fn cursor(&mut self) -> &mut dyn ICursor;
    fn display(&self) -> &dyn IDisplay;
    fn file_dialog(&mut self) -> &mut dyn IFileDialog;
    fn keyboard(&self) -> &dyn IKeyboard;
    fn text_input(&mut self) -> &mut dyn ITextInput;
    fn timer(&mut self) -> &mut dyn ITimer;
    fn notification(&mut self) -> &mut dyn INotification;
    fn console(&mut self) -> &mut dyn IConsole;
    fn file_system(&self) -> &dyn IFileSystem;
    fn system_info(&self) -> &dyn ISystemInfo;
}
