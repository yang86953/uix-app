// ============================================================================
// platform/shared/mod.rs — 跨平台共享核心实现
//
// 包含平台无关的共享实现：事件循环、文件系统、窗口状态与窗口核心。
// ============================================================================

pub mod event_loop;
pub mod filesystem;
pub mod state;
pub mod window;

pub use event_loop::OsEventSource;
pub use filesystem::{FileSystemCore, SpecialDirProvider};
pub use state::WindowState;
pub use window::{unimpl, PlatformWindowCore, WindowOps};
