//! 主事件循环。

#[allow(clippy::module_inception)]
pub mod event_loop;

pub use event_loop::run_widget_loop;
pub(crate) use event_loop::run_window_session_loop_with_system_theme_and_tasks;
