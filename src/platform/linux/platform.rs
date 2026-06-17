// ============================================================================
// platform/linux/platform.rs — Linux Platform facade (Wayland only)
// ============================================================================
//
// Uses native Wayland via wayland-client. Delegates window management to
// Box<dyn Backend>; owns shared subsystems that work independently of
// the display server protocol.
//
// Shared subsystems: console, file_system, system_info, file_dialog,
//                     notification, timer, text_input
// Backend-owned:     cursor, keyboard, display, clipboard
// ============================================================================

use crate::base::Point;
use crate::diag::Error;
use crate::platform::event::*;
use crate::platform::*;

use crate::platform::linux::backend::Backend;
use crate::platform::linux::console::LinuxConsole;
use crate::platform::linux::file_dialog::LinuxFileDialog;
use crate::platform::linux::filesystem::LinuxFileSystem;
use crate::platform::linux::notification::LinuxNotification;
use crate::platform::linux::system_info::LinuxSystemInfo;
use crate::platform::linux::text_input::LinuxTextInput;
use crate::platform::linux::timer::LinuxTimer;

// ════════════════════════════════════════════════════════════════════════════
// LinuxPlatform — Wayland-only facade
// ════════════════════════════════════════════════════════════════════════════

pub struct LinuxPlatform {
    // ── 后端（同时兼任 IPresenter）────────────────────────────────
    backend: Box<dyn Backend>,

    // ── 共享子系统─────────────────────────────────────────────────
    console_subsys: LinuxConsole,
    file_dialog_subsys: LinuxFileDialog,
    file_system_subsys: LinuxFileSystem,
    notification_subsys: LinuxNotification,
    system_info_subsys: LinuxSystemInfo,
    text_input_subsys: LinuxTextInput,
    timer_subsys: LinuxTimer,
}

// ════════════════════════════════════════════════════════════════════════════
// 构造——仅 Wayland
// ════════════════════════════════════════════════════════════════════════════

impl LinuxPlatform {
    pub fn new() -> Self {
        let (timer_tx, _timer_rx) = std::sync::mpsc::channel();

        let backend: Box<dyn Backend> = match crate::platform::linux::wayland::WaylandBackend::new()
        {
            Ok(wl) => {
                log::info!("Wayland backend initialized");
                Box::new(wl)
            }
            Err(e) => {
                panic!("Wayland backend failed: {}. X11 support was removed in 2026. Run under a Wayland session.", e);
            }
        };

        Self {
            backend,
            console_subsys: LinuxConsole::new(),
            file_dialog_subsys: LinuxFileDialog::new(),
            file_system_subsys: LinuxFileSystem::new(),
            notification_subsys: LinuxNotification::new(),
            system_info_subsys: LinuxSystemInfo::new(),
            text_input_subsys: LinuxTextInput::new(),
            timer_subsys: LinuxTimer::new(timer_tx),
        }
    }
}

impl Default for LinuxPlatform {
    fn default() -> Self {
        Self::new()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowManager — 委托给 backend
// ════════════════════════════════════════════════════════════════════════════

impl IWindowManager for LinuxPlatform {
    fn create_window(&mut self, title: &str, width: i32, height: i32) -> Result<(), Error> {
        log::info!("Creating window via Wayland ({}x{})", width, height);
        self.backend.create_window(title, width, height)
    }
    fn destroy_window(&mut self) {
        self.backend.destroy_window();
    }
    fn set_title(&mut self, title: &str) {
        self.backend.set_title(title);
    }
    fn show(&mut self) {
        self.backend.show();
    }
    fn hide(&mut self) {
        self.backend.hide();
    }
    fn is_visible(&self) -> bool {
        self.backend.is_visible()
    }
    fn center_on_screen(&mut self) {
        self.backend.center_on_screen();
    }
    fn raise(&mut self) {
        self.backend.raise();
    }
    fn lower(&mut self) {
        self.backend.lower();
    }
    fn set_window_icon(&mut self, icon_path: &str) {
        self.backend.set_window_icon(icon_path);
    }
    fn flash_window(&mut self) {
        self.backend.flash_window();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowProperties — 委托给 backend
// ════════════════════════════════════════════════════════════════════════════

impl IWindowProperties for LinuxPlatform {
    fn width(&self) -> i32 {
        self.backend.width()
    }
    fn height(&self) -> i32 {
        self.backend.height()
    }
    fn set_size(&mut self, w: i32, h: i32) {
        self.backend.set_size(w, h);
    }
    fn set_minimum_size(&mut self, w: i32, h: i32) {
        self.backend.set_minimum_size(w, h);
    }
    fn set_maximum_size(&mut self, w: i32, h: i32) {
        self.backend.set_maximum_size(w, h);
    }
    fn position(&self) -> Point {
        self.backend.position()
    }
    fn set_position(&mut self, x: i32, y: i32) {
        self.backend.set_position(x, y);
    }
    fn set_resizable(&mut self, resizable: bool) {
        self.backend.set_resizable(resizable);
    }
    fn is_maximized(&self) -> bool {
        self.backend.is_maximized()
    }
    fn is_minimized(&self) -> bool {
        self.backend.is_minimized()
    }
    fn maximize(&mut self) {
        self.backend.maximize();
    }
    fn minimize(&mut self) {
        self.backend.minimize();
    }
    fn restore(&mut self) {
        self.backend.restore();
    }
    fn set_borderless(&mut self, borderless: bool) {
        self.backend.set_borderless(borderless);
    }
    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.backend.set_fullscreen(fullscreen);
    }
    fn is_fullscreen(&self) -> bool {
        self.backend.is_fullscreen()
    }
    fn set_always_on_top(&mut self, on: bool) {
        self.backend.set_always_on_top(on);
    }
    fn set_window_opacity(&mut self, opacity: f32) {
        self.backend.set_window_opacity(opacity);
    }
    fn start_text_input(&mut self) {
        self.backend.start_text_input();
    }
    fn stop_text_input(&mut self) {
        self.backend.stop_text_input();
    }
    fn enable_file_drop(&mut self, enable: bool) {
        self.backend.enable_file_drop(enable);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IEventLoop — 委托给 backend
// ════════════════════════════════════════════════════════════════════════════

impl IEventLoop for LinuxPlatform {
    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        self.backend.poll_event(callback)
    }
    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        self.backend.wait_event(callback)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// INativeHandle — 委托给 backend
// ════════════════════════════════════════════════════════════════════════════

impl INativeHandle for LinuxPlatform {
    fn native_window(&self) -> *mut std::ffi::c_void {
        self.backend.native_window()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Platform — 组合接口
// ── backend 子系统 + 共享子系统
// ════════════════════════════════════════════════════════════════════════════

impl Platform for LinuxPlatform {
    fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        dirty_rect: Option<(i32, i32, i32, i32)>,
    ) {
        self.backend
            .present_pixels(pixels, width, height, dirty_rect);
    }
    fn presenter(&mut self) -> &mut dyn IPresenter {
        // Backend: IPresenter，所以 &mut dyn Backend 可直接协变到 &mut dyn IPresenter
        self.backend.as_mut()
    }
    fn clipboard(&mut self) -> &mut dyn IClipboard {
        &mut *self.backend
    }
    fn cursor(&mut self) -> &mut dyn ICursor {
        &mut *self.backend
    }
    fn display(&self) -> &dyn IDisplay {
        &*self.backend
    }
    fn keyboard(&self) -> &dyn IKeyboard {
        &*self.backend
    }
    fn file_dialog(&mut self) -> &mut dyn IFileDialog {
        &mut self.file_dialog_subsys
    }
    fn text_input(&mut self) -> &mut dyn ITextInput {
        &mut self.text_input_subsys
    }
    fn timer(&mut self) -> &mut dyn ITimer {
        &mut self.timer_subsys
    }
    fn notification(&mut self) -> &mut dyn INotification {
        &mut self.notification_subsys
    }
    fn console(&mut self) -> &mut dyn IConsole {
        &mut self.console_subsys
    }
    fn file_system(&self) -> &dyn IFileSystem {
        &self.file_system_subsys
    }
    fn system_info(&self) -> &dyn ISystemInfo {
        &self.system_info_subsys
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IPresenter — 委托给 backend
// ════════════════════════════════════════════════════════════════════════════

impl IPresenter for LinuxPlatform {
    fn present(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<(), Error> {
        self.backend.present(pixels, width, height)
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.backend.resize(width, height)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Drop
// ════════════════════════════════════════════════════════════════════════════

impl Drop for LinuxPlatform {
    fn drop(&mut self) {
        self.backend.destroy_window();
    }
}
