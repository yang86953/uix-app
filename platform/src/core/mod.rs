// ============================================================================
// platform/core/mod.rs — 平台共享核心模块
//
// 提供跨平台复用的状态、事件循环逻辑和窗口实现。
// 新平台只需实现 OsEventSource + WindowOps 即可获得完整功能。
// ============================================================================

pub mod state;
pub mod event_loop;
pub mod window;

pub use state::WindowState;
pub use event_loop::OsEventSource;
pub use window::{PlatformWindowCore, WindowOps, unimpl};
