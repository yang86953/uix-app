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

pub(crate) mod bindings;
// 剪贴板后端只供 crate 内部平台组合使用。
pub(crate) mod clipboard;
// Windows 常量只在 crate 内部实现间共享。
pub(crate) mod consts;
// 光标后端只供 crate 内部平台组合使用。
pub(crate) mod cursor;
pub(crate) mod custom_chrome;
// 显示信息后端只供 crate 内部平台组合使用。
pub(crate) mod display;
pub(crate) mod dpi;
pub(crate) mod ffi;
// 文件对话框后端只供 crate 内部平台组合使用。
pub(crate) mod file_dialog;
pub(crate) mod frame_pacer;
// GDI 呈现器只供 crate 内部图形组合使用。
pub(crate) mod gdi_presenter;
// Windows 辅助函数只在 crate 内部实现间共享。
pub(crate) mod helpers;
// 输入法派发只供 crate 内部窗口系统使用。
pub(crate) mod ime_dispatch;
// Windows 平台实现只由 crate 内部工厂构造。
pub(crate) mod platform;
// 系统信息后端只供 crate 内部服务使用。
#[path = "host/system_info/mod.rs"]
pub(crate) mod system_info;
// 文本输入后端只供 crate 内部窗口系统使用。
pub(crate) mod text_input;
mod tsf_document;
// TSF 会话只供 crate 内部文本输入组合使用。
pub(crate) mod tsf_session;
// TSF 文本存储只供 crate 内部文本输入组合使用。
pub(crate) mod tsf_text_store;
// Windows 通用工具只在 crate 内部实现间共享。
pub(crate) mod util;
mod window_icon;
// 窗口操作后端只供 crate 内部窗口系统使用。
pub(crate) mod window_ops;
// 非客户区交互模块集中映射跨平台缩放方向与 Win32 hit-test 消息。
pub(crate) mod window_interaction;
// 窗口过程只供 crate 内部平台实现注册。
pub(crate) mod wnd_proc;
