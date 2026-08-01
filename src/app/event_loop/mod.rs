//! 主事件循环与调度。
//!
//! # SMC 边界（SMC-06）
//!
//! event-loop Module 拥有主循环（event_loop）、帧调度（frame_scheduler）、
//! 时钟（clock）、主线程队列（main_thread_queue）、定时器（app_timer /
//! active_work_registry）；驱动 window 会话并注入 agent 状态。

#[allow(clippy::module_inception)]
pub(crate) mod event_loop;
pub(crate) mod frame_scheduler;
pub(crate) mod clock;
pub(crate) mod main_thread_queue;
pub(crate) mod app_timer;
pub(crate) mod active_work_registry;

pub use app_timer::TimerHandle;
pub use event_loop::run_widget_loop;
pub(crate) use event_loop::run_window_session_loop_with_system_theme_and_tasks;
