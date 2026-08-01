//! 应用能力 — 生命周期、主循环、窗口、CLI、DI。
//!
//! # SMC 边界（SMC-06）
//!
//! 本模块是 app System 的公开边界与私有实现。目标 Module 与依赖：
//!
//! | Module | 职责 | 依赖 |
//! |--------|------|------|
//! | [`application`] | 应用生命周期：App / AppMode / CLI / DI / AppHandle | event-loop、window、agent（全部单向） |
//! | [`event_loop`] | 主循环与调度：event loop / frame_scheduler / clock / main_thread_queue / app_timer / active_work_registry | window、agent（窗口会话驱动、agent 状态注入） |
//! | [`window`] | 窗口生命周期与会话：Window / window_driver / window_session / window_actions / window_config / text_input / bridge | agent（窗口事件入 agent 命令队列） |
//! | [`agent`] | 同用户本机 IPC 与自动化：agent_bridge / agent_control / agent_protocol / agent_transport（feature `agent-control` 相关） | 无（只依赖 System 私有边界） |
//!
//! System 私有边界（组合根与跨 Module 契约，任何 Module 不拥有）：
//!
//! | 归属 | 内容 |
//! |------|------|
//! | [`session_runtime`] | 组合根：AppRuntime / SessionRuntime 创建并注入各 Module 组件实例（agent bridge / transport / command queue、window session、event-loop waker） |
//! | [`window_semantics`] | 每窗口语义修订状态（event-loop / window / agent 共用） |
//!
//! 兄弟隔离审计（SMC-06，rg 证据）：agent 不引用任何兄弟 Module；window 只
//! 下行引用 agent；event-loop 只下行引用 window / agent；application 只下行
//! 引用 event-loop / window / agent；全部依赖经 System 私有边界（组合根注入）
//! 收敛，无环。

pub(crate) mod agent;
pub(crate) mod application;
pub(crate) mod event_loop;
pub(crate) mod session_runtime;
pub(crate) mod window;
pub(crate) mod window_semantics;

pub use crate::ui::AppState;
pub use application::app_handle::AppHandle;
pub use event_loop::app_timer::TimerHandle;
pub use application::application::{map_ui_event, App, AppMode};
pub use application::cli::{Cli, CliArgs};
pub use application::di::Container;
pub use window::window::Window;
pub use window::window_config::WindowConfig;
