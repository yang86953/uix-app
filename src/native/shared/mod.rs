// ============================================================================
// platform/shared/mod.rs — 跨平台共享核心实现
//
// 包含平台无关的共享实现：事件循环、文件系统、窗口状态与窗口核心。
// ============================================================================

#[allow(dead_code)]
pub(crate) mod buffer_lease;
pub mod event_loop;
pub mod filesystem;
pub mod ime_events;
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
pub mod state;
pub mod window;
pub mod window_lifecycle;
#[allow(dead_code)]
pub(crate) mod window_mode;
#[allow(dead_code)]
pub(crate) mod window_target;

pub use event_loop::OsEventSource;
pub use filesystem::{FileSystemCore, SpecialDirProvider};
pub use ime_events::{
    on_committed_text, on_committed_text_for_window, on_marked_text, on_marked_text_for_window,
    on_unmark_text, on_unmark_text_for_window, ImeCompositionState,
};
pub use state::WindowState;
pub use window::{unimpl, PlatformWindowCore, WindowOps};
pub use window_lifecycle::{push_window_close, push_window_resize};
