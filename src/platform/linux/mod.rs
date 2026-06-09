// ============================================================================
// platform/linux/mod.rs — Linux platform module entry point (Wayland only)
// ============================================================================
//
// Uses native Wayland via wayland-client. X11 support was removed in 2026.
// Cursor, display, and keyboard implementations are inlined in the
// WaylandBackend via Backend trait impl stubs.
// ============================================================================

#![cfg(all(unix, not(target_os = "macos")))]

pub(crate) mod backend;
pub mod clipboard;
pub mod console;
pub mod file_dialog;
pub mod filesystem;
pub mod notification;
pub mod platform;
pub mod system_info;
pub mod text_input;
pub mod timer;
pub(crate) mod wayland;

pub use clipboard::*;
pub use console::*;
pub use file_dialog::*;
pub use filesystem::*;
pub use notification::*;
pub use platform::*;
pub use system_info::*;
pub use text_input::*;
pub use timer::*;
