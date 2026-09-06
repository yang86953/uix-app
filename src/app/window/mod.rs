//! 应用级窗口管理。
//!
//! # SMC 边界（SMC-06）
//!
//! window Module 拥有窗口生命周期与会话：Window 公开模型、window_driver
//! （渲染/事件泵）、window_session（会话状态机）、window_actions（窗口动作
//! 窄适配）、window_config、text_input（输入法协调）与 bridge（UI↔Draw 桥接）。

// 保留 `window::bridge` 逻辑路径，呈现桥接物理归入 presentation_bridge。
#[path = "presentation_bridge/mod.rs"]
pub(crate) mod bridge;
pub(crate) mod frame_scheduler;
pub(crate) mod text_input;
#[allow(clippy::module_inception)]
pub(crate) mod window;
pub(crate) mod window_actions;
pub(crate) mod window_agent_ops;
pub(crate) mod window_config;
// 应用窗口创建 Adapter 统一注入 FileDrop 产品策略。
#[cfg(feature = "agent-control")]
pub(crate) mod background;
#[cfg(feature = "agent-control")]
pub(crate) mod offscreen_window;
pub(crate) mod window_creation;
pub(crate) mod window_driver;
pub(crate) mod window_session;
