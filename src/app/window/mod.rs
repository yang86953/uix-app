//! 应用级窗口管理。
//!
//! # SMC 边界（SMC-06）
//!
//! window Module 拥有窗口生命周期与会话：Window 公开模型、window_driver
//! （渲染/事件泵）、window_session（会话状态机）、window_actions（窗口动作
//! 窄适配）、window_config、text_input（输入法协调）与 bridge（UI↔Draw 桥接）。

#[allow(clippy::module_inception)]
pub mod window;
pub(crate) mod window_actions;
pub(crate) mod window_config;
pub(crate) mod window_driver;
pub(crate) mod window_session;
pub(crate) mod text_input;
pub(crate) mod bridge;

pub use window::Window;
pub use window_config::WindowConfig;
