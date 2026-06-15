// ============================================================================
// platform/platform.rs — 平台抽象接口（拆分为聚焦的子 trait + 聚合接口）
//
// 设计原则：
//   - 按职责拆分为 IWindowManager / IWindowProperties / IEventLoop 等
//   - Platform 作为聚合接口继承所有子 trait，保持向后兼容
//   - 子系统 accessor 保持组合模式
// ============================================================================

use crate::platform::event::*;
use crate::platform::types::*;
use crate::platform::*;

// ════════════════════════════════════════════════════════════════════════════
// IWindowManager — 窗口生命周期管理
// ════════════════════════════════════════════════════════════════════════════

pub trait IWindowManager {
    fn create_window(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> Result<(), crate::diag::Error>;
    fn destroy_window(&mut self);
    fn set_title(&mut self, title: &str);
    fn show(&mut self);
    fn hide(&mut self);
    fn is_visible(&self) -> bool;
    fn center_on_screen(&mut self);
    fn raise(&mut self);
    fn lower(&mut self);
    fn set_window_icon(&mut self, icon_path: &str);
    fn flash_window(&mut self);
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowProperties — 窗口属性查询与修改
// ════════════════════════════════════════════════════════════════════════════

pub trait IWindowProperties {
    fn width(&self) -> i32;
    fn height(&self) -> i32;
    fn set_size(&mut self, w: i32, h: i32);
    fn set_minimum_size(&mut self, w: i32, h: i32);
    fn set_maximum_size(&mut self, w: i32, h: i32);
    fn position(&self) -> Point;
    fn set_position(&mut self, x: i32, y: i32);
    fn set_resizable(&mut self, resizable: bool);
    fn is_maximized(&self) -> bool;
    fn is_minimized(&self) -> bool;
    fn maximize(&mut self);
    fn minimize(&mut self);
    fn restore(&mut self);
    fn set_borderless(&mut self, borderless: bool);
    fn set_fullscreen(&mut self, fullscreen: bool);
    fn is_fullscreen(&self) -> bool;
    fn set_always_on_top(&mut self, on: bool);
    fn set_window_opacity(&mut self, opacity: f32);
    fn start_text_input(&mut self);
    fn stop_text_input(&mut self);
    fn enable_file_drop(&mut self, enable: bool);
}

// ════════════════════════════════════════════════════════════════════════════
// IEventLoop — 事件循环
// ════════════════════════════════════════════════════════════════════════════

pub trait IEventLoop {
    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
}

// ════════════════════════════════════════════════════════════════════════════
// INativeHandle — 原生窗口句柄
// ════════════════════════════════════════════════════════════════════════════

pub trait INativeHandle {
    fn native_window(&self) -> *mut std::ffi::c_void;
}

// ════════════════════════════════════════════════════════════════════════════
// Platform — 聚合接口（保持向后兼容）
// ════════════════════════════════════════════════════════════════════════════

pub trait Platform: IWindowManager + IWindowProperties + IEventLoop + INativeHandle {
    // ── 像素呈现 ─────────────────────────────────────────────────

    /// Present a pixel buffer to the native window.
    ///
    /// `dirty_rect` 指定实际发生变化的区域（缓冲区坐标），
    /// 为 `None` 时表示全帧变化。平台层可用此信息做局部 damage，
    /// 减少合成器工作量。
    fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        dirty_rect: Option<(i32, i32, i32, i32)>,
    );

    /// Access the pixel presenter for direct presentation control.
    fn presenter(&mut self) -> &mut dyn IPresenter;

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
