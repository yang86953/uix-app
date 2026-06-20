// ============================================================================
// platform/linux/platform.rs — Linux Platform facade (Wayland only)
// ============================================================================
//
// Uses native Wayland via wayland-client. Directly owns WaylandBackend
// and implements all Platform subtraits by delegation.
//
// Owned shared subsystems: console, file_system, system_info, file_dialog,
//                          notification, text_input, timer
// Backend-owned:          window management, cursor, keyboard, display,
//                         clipboard, rendering
// ============================================================================

use crate::base::Point;
use crate::diag::Error;
use crate::platform::event::*;
use crate::platform::*;

use crate::platform::linux::console::LinuxConsole;
use crate::platform::linux::file_dialog::LinuxFileDialog;
use crate::platform::linux::filesystem::LinuxFileSystem;
use crate::platform::linux::notification::LinuxNotification;
use crate::platform::linux::system_info::LinuxSystemInfo;
use crate::platform::linux::text_input::LinuxTextInput;
use crate::platform::linux::timer::LinuxTimer;
use crate::platform::linux::wayland::WaylandBackend;

// ════════════════════════════════════════════════════════════════════════════
// LinuxPlatform — Wayland-only facade
// ════════════════════════════════════════════════════════════════════════════

pub struct LinuxPlatform {
    // ── Wayland 后端（窗口管理 + 呈现 + 光标/键盘/显示/剪贴板）──────
    backend: WaylandBackend,

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
// 构造
// ════════════════════════════════════════════════════════════════════════════

impl LinuxPlatform {
    pub fn new() -> Result<Self, Error> {
        let backend = match WaylandBackend::new() {
            Ok(wl) => {
                log::info!("Wayland backend initialized");
                wl
            }
            Err(e) => {
                return Err(Error::new(
                    crate::diag::Errc::PlatformError,
                    format!("Wayland backend failed: {}", e),
                ));
            }
        };

        let timer_eq = backend.event_queue_handle();

        Ok(Self {
            backend,
            console_subsys: LinuxConsole::new(),
            file_dialog_subsys: LinuxFileDialog::new(),
            file_system_subsys: LinuxFileSystem::new(),
            notification_subsys: LinuxNotification::new(),
            system_info_subsys: LinuxSystemInfo::new(),
            text_input_subsys: LinuxTextInput::new(),
            timer_subsys: LinuxTimer::new(timer_eq),
        })
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
    fn destroy_window(&mut self) { self.backend.destroy_window(); }
    fn set_title(&mut self, title: &str) { self.backend.set_title(title); }
    fn show(&mut self) { self.backend.show(); }
    fn hide(&mut self) { self.backend.hide(); }
    fn is_visible(&self) -> bool { self.backend.is_visible() }
    fn center_on_screen(&mut self) { self.backend.center_on_screen(); }
    fn raise(&mut self) { self.backend.raise(); }
    fn lower(&mut self) { self.backend.lower(); }
    fn set_window_icon(&mut self, icon_path: &str) { self.backend.set_window_icon(icon_path); }
    fn flash_window(&mut self) { self.backend.flash_window(); }
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowProperties — 委托给 backend
// ════════════════════════════════════════════════════════════════════════════

impl IWindowProperties for LinuxPlatform {
    fn width(&self) -> i32 { self.backend.width() }
    fn height(&self) -> i32 { self.backend.height() }
    fn set_size(&mut self, w: i32, h: i32) { self.backend.set_size(w, h); }
    fn set_minimum_size(&mut self, w: i32, h: i32) { self.backend.set_minimum_size(w, h); }
    fn set_maximum_size(&mut self, w: i32, h: i32) { self.backend.set_maximum_size(w, h); }
    fn position(&self) -> Point { self.backend.position() }
    fn set_position(&mut self, x: i32, y: i32) { self.backend.set_position(x, y); }
    fn set_resizable(&mut self, r: bool) { self.backend.set_resizable(r); }
    fn is_maximized(&self) -> bool { self.backend.is_maximized() }
    fn is_minimized(&self) -> bool { self.backend.is_minimized() }
    fn maximize(&mut self) { self.backend.maximize(); }
    fn minimize(&mut self) { self.backend.minimize(); }
    fn restore(&mut self) { self.backend.restore(); }
    fn set_borderless(&mut self, v: bool) { self.backend.set_borderless(v); }
    fn set_fullscreen(&mut self, v: bool) { self.backend.set_fullscreen(v); }
    fn is_fullscreen(&self) -> bool { self.backend.is_fullscreen() }
    fn set_always_on_top(&mut self, v: bool) { self.backend.set_always_on_top(v); }
    fn set_window_opacity(&mut self, v: f32) { self.backend.set_window_opacity(v); }
    fn start_text_input(&mut self) { self.backend.start_text_input(); }
    fn stop_text_input(&mut self) { self.backend.stop_text_input(); }
    fn enable_file_drop(&mut self, v: bool) { self.backend.enable_file_drop(v); }
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
// IPresenter — 委托给 backend
// ════════════════════════════════════════════════════════════════════════════

impl IPresenter for LinuxPlatform {
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        dirty_rect: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), Error> {
        self.backend.present(pixels, width, height, dirty_rect)
    }
    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.backend.resize(width, height)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ICursor — 委托给 backend
// ════════════════════════════════════════════════════════════════════════════

impl ICursor for LinuxPlatform {
    fn set_cursor(&mut self, cursor: crate::platform::types::CursorType) {
        self.backend.set_cursor(cursor);
    }
    fn show_cursor(&mut self, visible: bool) { self.backend.show_cursor(visible); }
    fn cursor_position(&self) -> Point { self.backend.cursor_position() }
    fn set_cursor_position(&mut self, x: i32, y: i32) { self.backend.set_cursor_position(x, y); }
    fn confine_cursor(&mut self, confine: bool) { self.backend.confine_cursor(confine); }
    fn capture_mouse(&mut self) { self.backend.capture_mouse(); }
    fn release_mouse(&mut self) { self.backend.release_mouse(); }
}

// ════════════════════════════════════════════════════════════════════════════
// IKeyboard — 委托给 backend
// ════════════════════════════════════════════════════════════════════════════

impl IKeyboard for LinuxPlatform {
    fn is_down(&self, key: crate::base::KeyCode) -> bool { self.backend.is_down(key) }
    fn idle_ms(&self) -> u32 { self.backend.idle_ms() }
    fn double_click_ms(&self) -> u32 { self.backend.double_click_ms() }
}

// ════════════════════════════════════════════════════════════════════════════
// IDisplay — 委托给 backend
// ════════════════════════════════════════════════════════════════════════════

impl IDisplay for LinuxPlatform {
    fn dpi_scale(&self) -> f32 { self.backend.dpi_scale() }
    fn is_dark_mode(&self) -> bool { self.backend.is_dark_mode() }
    fn count(&self) -> i32 { self.backend.count() }
    fn info(&self, index: i32) -> crate::platform::types::DisplayInfo { self.backend.info(index) }
}

// ════════════════════════════════════════════════════════════════════════════
// IClipboard — 委托给 backend
// ════════════════════════════════════════════════════════════════════════════

impl IClipboard for LinuxPlatform {
    fn text(&self) -> String { self.backend.text() }
    fn set_text(&mut self, text: &str) { self.backend.set_text(text); }
    fn has_text(&self) -> bool { self.backend.has_text() }
}

// ════════════════════════════════════════════════════════════════════════════
// Platform — 组合接口
// ════════════════════════════════════════════════════════════════════════════

impl Platform for LinuxPlatform {
    fn presenter(&mut self) -> &mut dyn IPresenter {
        self
    }
    fn clipboard(&mut self) -> &mut dyn IClipboard {
        self
    }
    fn cursor(&mut self) -> &mut dyn ICursor {
        self
    }
    fn display(&self) -> &dyn IDisplay {
        self
    }
    fn keyboard(&self) -> &dyn IKeyboard {
        self
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
// Drop
// ════════════════════════════════════════════════════════════════════════════

impl Drop for LinuxPlatform {
    fn drop(&mut self) {
        self.backend.destroy_window();
    }
}
