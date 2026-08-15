//! 主事件循环与调度。
//!
//! # SMC 边界（SMC-06）
//!
//! event-loop Module 拥有主循环（event_loop）、帧调度（frame_scheduler）、
//! 时钟（clock）、主线程队列（main_thread_queue）、定时器（app_timer /
//! active_work_registry）；驱动 window 会话并注入 agent 状态。

#[allow(clippy::module_inception)]
pub(crate) mod event_loop;
// 隔离 UI 光标请求到平台能力的去重交接。
mod pointer_cursor;

pub(crate) use event_loop::run_window_session_loop_with_system_theme_and_tasks;
