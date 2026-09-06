//! 应用能力 — 生命周期、主循环、窗口、CLI、DI。
//!
//! # SMC 边界（SMC-06）
//!
//! 本模块是 app System 的公开边界与私有实现。目标 Module 与依赖：
//!
//! | Module | 职责 | 依赖 |
//! |--------|------|------|
//! | `application` | 应用生命周期：App / AppMode / CLI / DI / AppHandle | event-loop、window、agent、queues（全部单向） |
//! | `event_loop` | 主循环：event loop（窗口会话驱动） | window、System 私有 queues（全部单向） |
//! | `window` | 窗口生命周期与会话：Window / window_driver / window_session / window_actions / window_config / text_input / bridge / frame_scheduler | System 私有 queues / window_semantics |
//! | `agent` | 同用户本机 IPC 与自动化：agent_bridge / agent_control / agent_protocol / agent_transport（feature `agent-control` 相关） | 无（只依赖 System 私有边界） |
//!
//! System 私有边界（组合根与跨 Module 契约，任何 Module 不拥有）：
//!
//! | 归属 | 内容 |
//! |------|------|
//! | `session_runtime` | 组合根：AppRuntime / SessionRuntime 创建并注入各 Module 组件实例（agent bridge / transport / command queue、window session、event-loop waker） |
//! | `window_semantics` | 每窗口语义修订状态（event-loop / window / agent 共用） |
//! | `queues` | 主循环与 Agent 命令的 System 私有契约/队列（clock / active_work_registry / app_timer / main_thread_queue / agent_command_queue / window_agent_state）：event-loop、window 与组合根共用，任何 Module 不拥有 |
//! | `app_events` | System 私有边界事件契约与总线（SystemEvent(SMC)：主题变更事实等）：任何 Module 不拥有，订阅进入路由清单 |
//!
//! 兄弟隔离现状（SMC-06，2026-08-03 全量审核修正，2026-08-04 P1 修复落地）：
//! agent 不引用任何兄弟 Module；application 只下行引用 event-loop / window /
//! agent / queues；event_loop / window / window_semantics 对 agent Module 的
//! 具体类型依赖已清零（rg 审计）：`WindowAgentState` / `AgentCommandQueue` /
//! 命令协议契约（`AgentCommandRequest` 等）上移 System 私有边界 `queues/`，
//! `agent` 只提供执行实现（`AgentCommandExecutor`）与语义发布端口实现
//! （`AgentSemanticsPort`，契约归 `window_semantics`），由组合根
//! `session_runtime` 组装期注入。

pub(crate) mod agent;
pub(crate) mod app_events;
pub(crate) mod application;
// Agent 协议的官方 Rust 客户端：服务端组装保持私有边界（SMC-06），
// 仅协议消费面公开，与 scripts/agent_client.py 同源同语义。
#[cfg(feature = "agent-control")]
pub mod agent_client;
#[cfg(feature = "agent-control")]
pub mod agent_workspace;
pub(crate) mod event_loop;
#[cfg(feature = "extensions")]
pub mod extensions;
pub(crate) mod queues;
pub(crate) mod session_runtime;
pub(crate) mod window;
pub(crate) mod window_semantics;

pub use crate::ui::AppState;
#[cfg(feature = "agent-control")]
pub use agent_client::AgentBridgeClient;
pub use application::app_handle::AppHandle;
pub use application::application::{App, AppMode, map_ui_event};
pub use application::cli::{Cli, CliArgs};
pub use application::di::Container;
pub use queues::agent_command_queue::AgentConfirmationRequest;
pub use queues::app_timer::TimerHandle;
pub use window::window::Window;
pub use window::window_config::{DesktopLayerConfig, WindowConfig, WindowSurfaceRole};
