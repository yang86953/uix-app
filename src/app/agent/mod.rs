//! 同用户本机 IPC 与自动化。
//!
//! # SMC 边界（SMC-06）
//!
//! agent Module 拥有 Agent Bridge / Agent Control / Agent Protocol /
//! Agent Transport（feature `agent-control` 相关组件）：只消费 System
//! 私有边界（`queues` 命令契约与执行契约、`window_semantics` 语义快照与
//! 发布端口，由组合根 `session_runtime` 组装期注入），不引用任何兄弟 Module。

pub(crate) mod agent_bridge;
pub(crate) mod agent_control;
pub(crate) mod agent_policy;
#[cfg(feature = "agent-control")]
pub(crate) mod agent_protocol;
#[cfg(feature = "agent-control")]
pub(crate) mod agent_screenshot;
#[cfg(feature = "agent-control")]
pub(crate) mod agent_transport;
