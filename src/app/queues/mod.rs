//! 主循环调度队列与跨 Module 契约 — app System 私有边界（SMC-06）。
//!
//! event-loop 与 window 两个 Module 共用的调度基础设施：
//!
//! | 模块 | 职责 |
//! |------|------|
//! | [`clock`] | 应用时钟抽象（wall / monotonic 注入点） |
//! | [`active_work_registry`] | 活跃工作注册表（窗口驱动循环的挂起工作追踪） |
//! | [`app_timer`] | 定时器队列（`TimerHandle` 的宿主） |
//! | [`main_thread_queue`] | 主线程任务队列 |
//! | [`agent_command_queue`] | Agent 命令契约与有界队列（SMC-06 P1 修复上移） |
//! | [`window_agent_state`] | 每窗口 Agent 命令状态机与执行契约（SMC-06 P1 修复上移） |
//!
//! 按 SMC「跨 Module 契约归 System 私有边界」收口：window 的窗口驱动循环与
//! event-loop 的主循环都消费本边界，二者之间不再互相引用（兄弟 Module 零依赖）。
//! 本边界只依赖 `crate::core` / `crate::draw` / `crate::ui` 的域外类型与同为
//! System 私有边界的 `window_semantics`，不依赖 app 内任何 Module；`agent`
//! Module 是本边界契约（`AgentCommandExecutor` / `AgentSemanticsPort`）的
//! 实现者，由组合根 `session_runtime` 组装期注入。

pub(crate) mod active_work_registry;
pub(crate) mod agent_command_queue;
pub(crate) mod app_timer;
pub(crate) mod clock;
pub(crate) mod main_thread_queue;
pub(crate) mod window_agent_state;
