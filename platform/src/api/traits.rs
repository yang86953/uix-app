//! # uix-platform 行为契约
//!
//! 本模块定义 platform 层的公开接口（trait）。
//! 各内部模块（linux/、windows/、shared/）实现这些接口。
//!
//! 外部使用者只依赖此处的接口签名，不依赖内部实现。

use super::types::{
    ConsoleColor, CursorType, DisplayInfo, Error, EventBus, KeyCode, MemoryInfo,
    OsInfo, Point, Result, SpecialDir, TerminalCapabilities, UiEvent,
};

// ════════════════════════════════════════════════════════════════════════════
// 像素呈现
// ════════════════════════════════════════════════════════════════════════════

/// CPU 像素呈现器接口。
pub trait IPresenter {
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        dirty_rect: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), Error>;
    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
}

/// GPU 图形上下文接口（GL 上下文生命周期管理）。
pub trait IGraphicsContext {
    fn initialize(
        &mut self,
        native_window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
    ) -> Result<(), Error>;
    fn resize(&mut self, width: i32, height: i32);
    fn make_current(&mut self);
    fn swap_buffers(&mut self);
    fn shutdown(&mut self);
    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Vec<u32>;
    fn width(&self) -> i32;
    fn height(&self) -> i32;

    fn get_proc_address(&self, name: &str) -> Option<*const std::ffi::c_void> {
        let _ = name;
        None
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 窗口属性与原生句柄
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

pub trait INativeHandle {
    fn native_window(&self) -> *mut std::ffi::c_void;
}

pub trait IWindowManager {
    fn create_window(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> Result<Box<dyn PlatformWindow>, Error>;
}

// ════════════════════════════════════════════════════════════════════════════
// 窗口
// ════════════════════════════════════════════════════════════════════════════

pub trait PlatformWindow {
    fn show(&mut self);
    fn hide(&mut self);
    fn close(&mut self);
    fn is_visible(&self) -> bool;
    fn set_title(&mut self, title: &str);
    fn center_on_screen(&mut self);
    fn raise(&mut self);
    fn lower(&mut self);
    fn set_window_icon(&mut self, icon_path: &str);
    fn flash_window(&mut self);
    fn resize_notify(&mut self, width: i32, height: i32);
    fn properties(&self) -> &dyn IWindowProperties;
    fn properties_mut(&mut self) -> &mut dyn IWindowProperties;
    fn presenter(&mut self) -> &mut dyn IPresenter;
    fn native_handle(&self) -> &dyn INativeHandle;

    fn graphics_context(&mut self) -> Option<&mut dyn IGraphicsContext> {
        None
    }

    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 事件循环
// ════════════════════════════════════════════════════════════════════════════

pub trait IEventLoop {
    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
    fn wait_timeout(
        &mut self,
        timeout: std::time::Duration,
        callback: &dyn Fn(&UiEvent) -> bool,
    ) -> bool;
}

// ════════════════════════════════════════════════════════════════════════════
// 输入
// ════════════════════════════════════════════════════════════════════════════

pub trait IClipboard {
    fn text(&self) -> String;
    fn set_text(&mut self, text: &str);
    fn has_text(&self) -> bool;
}

pub trait ICursor {
    fn set_cursor(&mut self, cursor: CursorType);
    fn show_cursor(&mut self, visible: bool);
    fn cursor_position(&self) -> Point;
    fn set_cursor_position(&mut self, x: i32, y: i32);
    fn confine_cursor(&mut self, confine: bool);
    fn capture_mouse(&mut self);
    fn release_mouse(&mut self);
}

pub trait ITextInput {
    fn start(&mut self);
    fn stop(&mut self);
}

pub trait IKeyboard {
    fn is_down(&self, key: KeyCode) -> bool;
    fn idle_ms(&self) -> u32;
    fn double_click_ms(&self) -> u32;
}

// ════════════════════════════════════════════════════════════════════════════
// 显示
// ════════════════════════════════════════════════════════════════════════════

pub trait IDisplay {
    fn dpi_scale(&self) -> f32;
    fn is_dark_mode(&self) -> bool;
    fn count(&self) -> i32;
    fn info(&self, index: i32) -> DisplayInfo;
}

// ════════════════════════════════════════════════════════════════════════════
// 控制台
// ════════════════════════════════════════════════════════════════════════════

pub trait IConsole {
    fn write(&mut self, text: &str);
    fn write_line(&mut self, text: &str);
    fn set_color(&mut self, color: ConsoleColor);
    fn reset_color(&mut self);
    fn show_terminal_cursor(&mut self, visible: bool);
    fn set_terminal_title(&mut self, title: &str);
    fn capabilities(&self) -> TerminalCapabilities;
}

// ════════════════════════════════════════════════════════════════════════════
// 系统服务
// ════════════════════════════════════════════════════════════════════════════

pub trait IFileDialog {
    fn open(&mut self, title: &str, filters: &str) -> Vec<String>;
    fn save(&mut self, title: &str, filters: &str) -> String;
    fn open_folder(&mut self, title: &str) -> String;
}

pub trait IFileSystem {
    fn get_special_dir(&self, dir: SpecialDir) -> String;
    fn executable_path(&self) -> String;
    fn executable_dir(&self) -> String;
    fn read_file(&self, path: &str) -> Result<Vec<u8>, Error>;
}

pub trait INotification {
    fn show(&mut self, title: &str, message: &str);
}

pub trait ITimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> u32;
    fn clear(&mut self, id: u32);
}

pub trait ISystemInfo {
    fn os_info(&self) -> OsInfo;
    fn cpu_count(&self) -> u32;
    fn memory_info(&self) -> MemoryInfo;
    fn hostname(&self) -> String;
    fn username(&self) -> String;
    fn up_time(&self) -> u64;
    fn default_font_path(&self) -> Option<String>;
    fn default_font_paths(&self) -> Vec<String> {
        self.default_font_path().into_iter().collect()
    }
    fn probe_cjk_font_path(&self) -> Option<String> {
        None
    }
    fn probe_family_font_path(&self, _family: &str) -> Option<String> {
        None
    }
    fn scan_fallback_font_path(&self) -> Option<String> {
        None
    }
    fn process_memory(&self) -> (usize, usize) {
        (0, 0)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 平台聚合
// ════════════════════════════════════════════════════════════════════════════

pub trait Platform {
    fn window_manager(&mut self) -> &mut dyn IWindowManager;
    fn event_loop(&mut self) -> &mut dyn IEventLoop;
    fn event_bus(&mut self) -> &mut EventBus;
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
