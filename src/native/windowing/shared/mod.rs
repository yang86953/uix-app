// ============================================================================
// platform/shared/mod.rs — 跨平台共享核心实现
//
// 包含平台无关的共享实现：事件循环、文件系统、窗口状态与窗口核心。
// ============================================================================

#[allow(dead_code)]
pub(crate) mod buffer_lease;
pub(crate) mod event_loop;
pub(crate) mod ime_events;
#[allow(dead_code)]
pub(crate) mod ime_owner;
#[allow(dead_code)]
pub(crate) mod input_serial;
#[allow(dead_code)]
pub(crate) mod native_frame_mailbox;
#[allow(dead_code)]
pub(crate) mod nonblocking_read;
#[allow(dead_code)]
pub(crate) mod nonblocking_write;
pub(crate) mod state;
pub(crate) mod window;
pub(crate) mod window_lifecycle;
#[allow(dead_code)]
pub(crate) mod window_mode;
#[allow(dead_code)]
pub(crate) mod window_target;

pub(crate) use event_loop::OsEventSource;
pub(crate) use state::WindowState;
pub(crate) use window::PlatformWindowCore;
