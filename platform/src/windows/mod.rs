// ============================================================================
// platform/windows/mod.rs — Windows 平台实现入口
//
// 新架构：
//   platform.rs    — WindowsPlatform（OsEventSource + IWindowManager + Platform + wnd_proc）
//   window_ops.rs  — WindowsWindowOps（实现 WindowOps trait）
//   各子系统保持独立文件。
// ============================================================================

#![cfg(windows)]
#![allow(non_snake_case)]

pub mod bindings;
pub mod clipboard;
pub mod console;
pub mod consts;
pub mod cursor;
pub mod display;
pub mod ffi;
pub mod file_dialog;
pub mod filesystem;
pub mod gdi_presenter;
pub mod helpers;
pub mod keyboard;
pub mod notification;
pub mod platform;
pub mod system_info;
pub mod text_input;
pub mod timer;
pub mod util;
pub mod window_ops;
pub mod wnd_proc;

pub use clipboard::*;
pub use console::*;
pub use cursor::*;
pub use display::*;
pub use file_dialog::*;
pub use filesystem::*;
pub use gdi_presenter::*;
pub use keyboard::*;
pub use notification::*;
pub use platform::*;
pub use system_info::*;
pub use text_input::*;
pub use timer::*;
