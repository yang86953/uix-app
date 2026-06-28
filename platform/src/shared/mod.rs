// ============================================================================
// platform/shared/mod.rs — 跨平台共享核心实现
//
// 包含平台无关的共享实现和所有窗口/事件相关的 API trait 定义。
// ============================================================================

pub mod state;
pub mod event_loop;
pub mod window;

pub use state::WindowState;
pub use event_loop::OsEventSource;
pub use window::{
    PlatformWindowCore, WindowOps, unimpl,
};
