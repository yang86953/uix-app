// ============================================================================
// platform/linux/mod.rs — Linux 平台入口（Wayland）
// ============================================================================

#![cfg(target_os = "linux")]

pub(crate) mod file_dialog;
pub(crate) mod platform;
#[path = "host/system_info/mod.rs"]
pub(crate) mod system_info;
#[path = "windowing/wayland/mod.rs"]
pub(crate) mod wayland;
