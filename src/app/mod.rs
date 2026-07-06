//! 应用能力 — 生命周期、主循环、窗口、CLI、DI。

pub(crate) mod active_work_registry;
pub mod app_handle;
pub mod app_timer;
pub mod bridge;
pub mod event_loop;
pub(crate) mod main_thread_queue;
pub(crate) mod session_runtime;
pub mod shell;
pub(crate) mod test_clock;
pub mod window;
pub(crate) mod window_session;

pub use crate::ui::AppState;
pub use app_handle::{AppHandle, WindowId};
pub use app_timer::TimerHandle;
pub use shell::application::{map_ui_event, App, AppMode};
pub use shell::cli::{Cli, CliArgs};
pub use shell::di::Container;
pub use window::window::Window;
