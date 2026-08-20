// ============================================================================
// platform/linux/mod.rs — Linux 平台入口（Wayland）
// ============================================================================

#![cfg(all(unix, not(target_os = "macos")))]

pub(crate) mod console;
pub(crate) mod file_dialog;
pub(crate) mod filesystem;
pub(crate) mod notification;
pub(crate) mod platform;
#[path = "host/system_info/mod.rs"]
pub(crate) mod system_info;
pub(crate) mod timer;
#[path = "windowing/wayland/mod.rs"]
pub(crate) mod wayland;
