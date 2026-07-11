// ============================================================================
// platform/windows/mod.rs — Windows 平台实现入口
//
// 架构：
//   platform.rs    — WindowsPlatform（OsEventSource + IWindowManager + Platform + wnd_proc）
//   window_ops.rs  — WindowsWindowOps（实现 WindowOps trait）
//   gdi_presenter/ — GDI 像素呈现
//   system_info/   — 系统信息
//   infra/         — FFI 绑定、常量、工具函数（bindings/consts/ffi/helpers/util）
//   其余文件为各子系统实现。
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
pub mod ime_dispatch;
pub mod keyboard;
pub mod notification;
pub mod platform;
pub mod system_info;
pub mod text_input;
pub mod timer;
pub mod tsf_session;
pub mod tsf_text_store;
pub mod util;
pub mod window_ops;
pub mod wnd_proc;
