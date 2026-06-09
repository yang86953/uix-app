//! UIX demo programs — CLI and GUI demonstrations of the UIX framework.
//! These are binary-only modules (not part of the `uix` library crate).

pub mod cli;

// GUI demo is Windows-only (uses Win32 API directly)
#[cfg(windows)]
pub mod gui;

// Linux GUI demo uses Wayland (separate module)
#[cfg(all(unix, not(target_os = "macos")))]
pub mod gui_linux;
