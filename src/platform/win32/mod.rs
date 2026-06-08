// ============================================================================
// uix-platform/src/win32/mod.rs — Win32 platform implementation entry point
// ============================================================================

#![cfg(windows)]
#![allow(non_snake_case)]

pub mod console;
pub mod filesystem;
pub mod gdi_presenter;
pub mod platform;
pub mod system_info;
pub mod util;

pub use console::*;
pub use filesystem::*;
pub use gdi_presenter::*;
pub use platform::*;
pub use system_info::*;
