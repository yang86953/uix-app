// ============================================================================
// platform/mod.rs — 平台能力接口（聚合入口）
// ============================================================================

// ── 核心平台模块 ──────────────────────────────────────────────────
pub mod abstraction;
pub mod event;
pub mod types;

#[cfg(windows)]
pub mod win32;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod linux;

// ── 子系统 trait 模块 ─────────────────────────────────────────────
pub mod clipboard;
pub mod console;
pub mod cursor;
pub mod display;
pub mod file_dialog;
pub mod file_system;
pub mod input;
pub mod notification;
pub mod system_info;
pub mod timer;

// ── 公开重导出 ────────────────────────────────────────────────────
pub use clipboard::*;
pub use console::*;
pub use cursor::*;
pub use display::*;
pub use event::*;
pub use file_dialog::*;
pub use file_system::*;
pub use input::*;
pub use notification::*;
pub use system_info::*;
pub use timer::*;

// ── Platform 子 trait 重导出 ──────────────────────────────────────
pub use abstraction::{IEventLoop, INativeHandle, IWindowManager, IWindowProperties, Platform};

// ── 工厂函数 ──────────────────────────────────────────────────────

#[cfg(windows)]
pub fn create_platform() -> Box<dyn Platform> {
    Box::new(win32::Win32Platform::new())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn create_platform() -> Box<dyn Platform> {
    Box::new(linux::LinuxPlatform::new())
}

#[cfg(target_os = "macos")]
compile_error!("uix-platform does not yet support macOS targets");
