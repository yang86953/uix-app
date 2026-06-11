// ============================================================================
// platform/api.rs — Platform 层的公共 API 出口
//
// 本文件定义 platform 层对外暴露的公共接口。其他层只能通过本文件
// 使用 platform 层的功能，禁止直接引用内部模块。
// ============================================================================

// ── 平台抽象接口 ──
pub use crate::platform::abstraction::{
    IEventLoop, INativeHandle, IWindowManager, IWindowProperties, Platform,
};

// ── 像素呈现 ──
pub use crate::platform::presenter::{IPresenter, NullPresenter};

// ── 基础类型 ──
pub use crate::platform::types::{KeyCode, KeyMod, MouseButton};

// ── 事件类型 ──
pub use crate::platform::event::{
    FileDropData, KeyEventData, KeyPressData, MouseButtonEventData, MouseMoveEventData,
    MouseWheelData, ResizeData, TimerEventData, UiEvent, UiEventPayload, UiEventType,
};

// ── 子系统 Trait ──
pub use crate::platform::clipboard::IClipboard;
pub use crate::platform::console::{ConsoleColor, IConsole, TerminalCapabilities};
pub use crate::platform::cursor::{CursorType, ICursor};
pub use crate::platform::display::{DisplayInfo, IDisplay};
pub use crate::platform::file_dialog::IFileDialog;
pub use crate::platform::file_system::{IFileSystem, SpecialDir};
pub use crate::platform::input::{IKeyboard, ITextInput};
pub use crate::platform::notification::INotification;
pub use crate::platform::system_info::{ISystemInfo, MemoryInfo, OsInfo};
pub use crate::platform::timer::ITimer;

// ── 工厂函数 ──
#[cfg(windows)]
pub fn create_platform() -> Box<dyn Platform> {
    Box::new(crate::platform::win32::Win32Platform::new())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn create_platform() -> Box<dyn Platform> {
    Box::new(crate::platform::linux::LinuxPlatform::new())
}

#[cfg(target_os = "macos")]
compile_error!("uix-platform does not yet support macOS targets");