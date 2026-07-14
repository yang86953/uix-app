//! 应用能力 — 生命周期、主循环、窗口、CLI、DI。

pub(crate) mod active_work_registry;
pub(crate) mod agent_bridge;
pub(crate) mod agent_control;
#[cfg(feature = "agent-control")]
pub(crate) mod agent_protocol;
#[cfg(feature = "agent-control")]
pub(crate) mod agent_transport;
pub mod app_handle;
pub mod app_timer;
pub mod bridge;
pub(crate) mod clock;
pub mod event_loop;
pub(crate) mod frame_scheduler;
pub(crate) mod main_thread_queue;
pub(crate) mod session_runtime;
pub mod shell;
pub(crate) mod text_input;
pub mod window;
pub(crate) mod window_actions;
pub mod window_config;
pub(crate) mod window_driver;
pub(crate) mod window_semantics;
pub(crate) mod window_session;

pub use crate::ui::AppState;
pub use app_handle::{AppHandle, WindowId};
pub use app_timer::TimerHandle;
pub use shell::application::{map_ui_event, App, AppMode};
pub use shell::cli::{Cli, CliArgs};
pub use shell::di::Container;
pub use window::window::Window;
pub use window_config::WindowConfig;
