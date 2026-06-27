// ============================================================================
// platform/api.rs — 平台层统一 API 契约
//
// 设计原则：
//   每个行为类别定义一个 trait，所有 trait 集中在此文件，作为平台层的
//   "通用 API" 契约。各平台（linux / windows）只需 impl 这些 trait。
//   共享数据类型在 crate::types 中定义。
//
// 多窗口架构：
//   - PlatformWindow：每个窗口的独立操作（show/hide/present/属性查询等）
//   - Platform：共享资源（事件循环、剪贴板、显示器、输入等） + 窗口工厂
//   - IWindowProperties：窗口属性查询/修改（PlatformWindow 的子组件）
// ============================================================================

pub use crate::error::*;
pub use crate::geometry::*;
pub use crate::status::*;
pub use crate::types::{KeyCode, KeyMod, MouseButton};
use crate::event::*;
use crate::types::*;

// ════════════════════════════════════════════════════════════════════════════
// IWindowProperties — 窗口属性查询与修改（PlatformWindow 的子组件）
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
    /// 返回系统默认字体文件路径列表（用于回退链）。
    fn default_font_paths(&self) -> Vec<String> {
        self.default_font_path().into_iter().collect()
    }
    /// 探测 CJK 回退字体路径（不支持的平台返回 None）。
    fn probe_cjk_font_path(&self) -> Option<String> { None }
    /// 通过字体族名称查找字体路径（不支持的平台返回 None）。
    fn probe_family_font_path(&self, _family: &str) -> Option<String> { None }
    /// 获取进程内存使用信息。返回 (工作集字节, 私有字节)。
    /// 不支持时返回 (0, 0)。
    fn process_memory(&self) -> (usize, usize) { (0, 0) }
}

// ════════════════════════════════════════════════════════════════════════════
// IPresenter — 像素呈现（每个窗口独立）
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
// IGraphicsContext — GPU 图形上下文（平台层管理 GL 上下文生命周期）
//
// GpuEngine 通过此 trait 获取 GL 上下文，直接使用 glow 进行 GPU 渲染。
// platform 层只负责上下文生命周期（创建/激活/交换/销毁），
// graphics 层负责所有着色器、纹理、绘制逻辑。
// ════════════════════════════════════════════════════════════════════════════

pub trait IGraphicsContext {
    /// 初始化 GL 上下文，绑定到指定原生窗口。
    /// `native_window` 是平台原生窗口句柄（如 Wayland wl_surface*、Windows HWND）。
    fn initialize(
        &mut self,
        native_window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
    ) -> Result<(), Error>;

    /// 调整为新的帧缓冲尺寸（窗口 resize 后调用）。
    fn resize(&mut self, width: i32, height: i32);

    /// 使此上下文成为当前线程的活跃 GL 上下文。
    /// GpuEngine 在每帧开始前必须调用此方法。
    fn make_current(&mut self);

    /// 交换前后缓冲区，将渲染结果呈现到窗口。
    fn swap_buffers(&mut self);

    /// 销毁 GL 上下文，释放所有 GPU 资源。
    fn shutdown(&mut self);

    /// 从默认帧缓冲读回像素（用于 CPU fallback 或诊断）。
    /// 返回 BGRA 格式的 u32 像素数组。
    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Vec<u32>;

    /// 当前帧缓冲宽度。
    fn width(&self) -> i32;

    /// 当前帧缓冲高度。
    fn height(&self) -> i32;

    /// 获取 GL 函数指针（用于 glow 等 GL 绑定库加载函数）。
    /// 返回 `None` 表示该函数不可用。
    fn get_proc_address(&self, name: &str) -> Option<*const std::ffi::c_void> {
        let _ = name;
        None
    }
}

// ════════════════════════════════════════════════════════════════════════════
// INativeHandle — 原生窗口句柄（每个窗口独立）
// ════════════════════════════════════════════════════════════════════════════

pub trait INativeHandle {
    fn native_window(&self) -> *mut std::ffi::c_void;
}

// ════════════════════════════════════════════════════════════════════════════
// IEventLoop — 事件循环（平台共享）
// ════════════════════════════════════════════════════════════════════════════

pub trait IEventLoop {
    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
    /// Block until an event arrives or the timeout expires.
    fn wait_timeout(&mut self, timeout: std::time::Duration, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowManager — 窗口工厂（平台共享，创建 PlatformWindow）
// ════════════════════════════════════════════════════════════════════════════

pub trait IWindowManager {
    /// 创建一个新窗口，返回窗口级操作句柄。
    fn create_window(&mut self, title: &str, width: i32, height: i32) -> Result<Box<dyn PlatformWindow>, Error>;
}

// ════════════════════════════════════════════════════════════════════════════
// PlatformWindow — 单窗口操作接口（多窗口架构核心）
//
// 每个窗口是一个独立的 trait object，封装：
//   - 窗口生命周期（show/hide/close）
//   - 窗口外观（title/icon/flash）
//   - 窗口属性（IWindowProperties 子组件）
//   - 像素呈现（IPresenter 子组件）
//   - 原生句柄（INativeHandle 子组件）
//
// 共享资源（事件循环、剪贴板等）通过 Platform trait 访问。
// ════════════════════════════════════════════════════════════════════════════

pub trait PlatformWindow {
    // ── 窗口生命周期 ──────────────────────────────────────────
    fn show(&mut self);
    fn hide(&mut self);
    fn close(&mut self);
    fn is_visible(&self) -> bool;

    // ── 窗口外观 ──────────────────────────────────────────────
    fn set_title(&mut self, title: &str);
    fn center_on_screen(&mut self);
    fn raise(&mut self);
    fn lower(&mut self);
    fn set_window_icon(&mut self, icon_path: &str);
    fn flash_window(&mut self);

    // ── 几何通知 ──────────────────────────────────────────────
    /// 通知窗口尺寸已变化（用于同步平台资源如 SHM 缓冲）。
    fn resize_notify(&mut self, width: i32, height: i32);

    // ── 子组件访问 ────────────────────────────────────────────
    fn properties(&self) -> &dyn IWindowProperties;
    fn properties_mut(&mut self) -> &mut dyn IWindowProperties;
    fn presenter(&mut self) -> &mut dyn IPresenter;
    fn native_handle(&self) -> &dyn INativeHandle;

    /// GPU 图形上下文。仅 GPU/Hybrid 模式可用，CPU 模式返回 `None`。
    fn graphics_context(&mut self) -> Option<&mut dyn IGraphicsContext> { None }

    /// Wayland wl_surface 原始指针（供 EGL 初始化）。非 Wayland 平台返回 null。
    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Platform — 聚合接口（组合模式）
//
// 窗口专属操作移到 PlatformWindow trait。
// 共享资源（事件循环、输入、系统服务等）保留在此。
// ════════════════════════════════════════════════════════════════════════════

pub trait Platform {
    // ── 窗口工厂 ─────────────────────────────────────────────
    fn window_manager(&mut self) -> &mut dyn IWindowManager;

    // ── 事件循环（平台共享）─────────────────────────────────
    fn event_loop(&mut self) -> &mut dyn IEventLoop;

    // ── 子系统访问器（平台共享）─────────────────────────────
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
