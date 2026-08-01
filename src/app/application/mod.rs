//! 应用壳 — 生命周期与 CLI/DI。
//!
//! # SMC 边界（SMC-06）
//!
//! application Module 拥有应用生命周期（App / AppMode）、CLI 入口、DI 容器
//! 与应用句柄（AppHandle）；组合与注入由 System 私有边界
//! `session_runtime` 承担。

pub(crate) mod app_handle;
pub mod application;
pub mod cli;
pub mod di;

pub use application::{map_ui_event, App, AppMode};
pub use cli::{Cli, CliArgs};
pub use di::Container;
