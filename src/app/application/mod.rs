//! 应用壳 — 生命周期与 CLI/DI。
//!
//! # SMC 边界（SMC-06）
//!
//! application Module 拥有应用生命周期（App / AppMode）、CLI 入口、DI 容器
//! 与应用句柄（AppHandle）；组合与注入由 System 私有边界
//! `session_runtime` 承担。

pub(crate) mod app_handle;
pub(crate) mod application;
pub(crate) mod cli;
pub(crate) mod di;
// 保存 Application System 私有的逐窗反馈 owner 与声明租约表。
pub(crate) mod extensions;
// 保存 UIX 文档生成的 App 作用域具名主题表。
mod named_themes;
