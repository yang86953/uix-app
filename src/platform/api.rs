// ============================================================================
// platform/api.rs — 平台层统一 API 契约
//
// 设计原则：
//   每个行为类别定义一个 trait，所有 trait 集中在此文件，作为平台层的
//   "通用 API" 契约。各平台（linux / windows）只需 impl 这些 trait。
//   共享数据类型在 crate::base 中定义。
// ============================================================================

use crate::base::*;
use crate::diag::Error;
use crate::platform::event::*;
use crate::platform::types::*;

// ════════════════════════════════════════════════════════════════════════════
// IClipboard — 剪贴板
// ════════════════════════════════════════════════════════════════════════════

pub trait IClipboard {
    fn text(&self) -> String;
    fn set_text(&mut self, text: &str);
    fn has_text(&self) -> bool;
}

// ════════════════════════════════════════════════════════════════════════════
// IConsole — 终端控制台
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
// ICursor — 光标
// ════════════════════════════════════════════════════════════════════════════

pub trait ICursor {
    fn set_cursor(&mut self, cursor: CursorType);
    fn show_cursor(&mut self, visible: bool);
    fn cursor_position(&self) -> Point;
    fn set_cursor_position(&mut self, x: i32, y: i32);
    fn confine_cursor(&mut self, confine: bool);
    fn capture_mouse(&mut self);
    fn release_mouse(&mut self);
}

// ════════════════════════════════════════════════════════════════════════════
// IDisplay — 显示器
// ════════════════════════════════════════════════════════════════════════════

pub trait IDisplay {
    fn dpi_scale(&self) -> f32;
    fn is_dark_mode(&self) -> bool;
    fn count(&self) -> i32;
    fn info(&self, index: i32) -> DisplayInfo;
}

// ════════════════════════════════════════════════════════════════════════════
// IFileDialog — 文件对话框
// ════════════════════════════════════════════════════════════════════════════

pub trait IFileDialog {
    fn open(&mut self, title: &str, filters: &str) -> Vec<String>;
    fn save(&mut self, title: &str, filters: &str) -> String;
    fn open_folder(&mut self, title: &str) -> String;
}

// ════════════════════════════════════════════════════════════════════════════
// IFileSystem — 文件系统
// ════════════════════════════════════════════════════════════════════════════

pub trait IFileSystem {
    fn get_special_dir(&self, dir: SpecialDir) -> String;
    fn executable_path(&self) -> String;
    fn executable_dir(&self) -> String;
    fn read_file(&self, path: &str) -> Result<Vec<u8>, Error>;
}

// ════════════════════════════════════════════════════════════════════════════
// IKeyboard — 键盘状态查询
// ════════════════════════════════════════════════════════════════════════════

pub trait IKeyboard {
    fn is_down(&self, key: KeyCode) -> bool;
    fn idle_ms(&self) -> u32;
    fn double_click_ms(&self) -> u32;
}

// ════════════════════════════════════════════════════════════════════════════
// ITextInput — 文本输入（IME）控制
// ════════════════════════════════════════════════════════════════════════════

pub trait ITextInput {
    fn start(&mut self);
    fn stop(&mut self);
}

// ════════════════════════════════════════════════════════════════════════════
// INotification — 系统通知
// ════════════════════════════════════════════════════════════════════════════

pub trait INotification {
    /// 显示系统通知
    fn show(&mut self, title: &str, message: &str);
}

// ════════════════════════════════════════════════════════════════════════════
// ITimer — 定时器
// ════════════════════════════════════════════════════════════════════════════

pub trait ITimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> u32;
    fn clear(&mut self, id: u32);
}

// ════════════════════════════════════════════════════════════════════════════
// ISystemInfo — 系统信息
// ════════════════════════════════════════════════════════════════════════════

pub trait ISystemInfo {
    fn os_info(&self) -> OsInfo;
    fn cpu_count(&self) -> u32;
    fn memory_info(&self) -> MemoryInfo;
    fn hostname(&self) -> String;
    fn username(&self) -> String;
    fn up_time(&self) -> u64;
    /// 返回系统当前默认字体文件路径（如有）。
    fn default_font_path(&self) -> Option<String>;
}

// ════════════════════════════════════════════════════════════════════════════
// IPresenter — 像素呈现
// ════════════════════════════════════════════════════════════════════════════

pub trait IPresenter {
    /// 将 ARGB 像素缓冲区呈现到窗口。
    /// `dirty_rect` 为局部更新区域 `(x, y, w, h)`，`None` 表示全帧。
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        dirty_rect: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), Error>;
    /// 窗口尺寸变化时重建中间资源
    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowManager — 窗口生命周期管理
// ════════════════════════════════════════════════════════════════════════════

pub trait IWindowManager {
    fn create_window(&mut self, title: &str, width: i32, height: i32) -> Result<(), Error>;
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
// Platform — 聚合接口（组合模式）
//
// 所有子系统统一通过访问器方法暴露，不使用 trait 继承。
// 平台实现方通过组合持有各子系统 struct，在访问器中返回对应引用。
// ════════════════════════════════════════════════════════════════════════════

pub trait Platform {
    // ── 核心窗口访问器（组合模式）─────────────────────────────────
    fn window_manager(&mut self) -> &mut dyn IWindowManager;
    fn window_properties(&self) -> &dyn IWindowProperties;
    fn event_loop(&mut self) -> &mut dyn IEventLoop;
    fn native_handle(&self) -> &dyn INativeHandle;
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
