#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

//! UIX — Rust Native UI Framework
//!
//! # 功能域
//!
//! | 模块 | 职责 |
//! |------|------|
//! | [`core`] | 基础设施 — 错误、几何、日志、诊断 |
//! | [`native`] | 平台能力 — OS 抽象（Win32 / Wayland） |
//! | [`draw`] | 绘制能力 — 2D 引擎、光栅化、字体、合成 |
//! | [`ui`] | 界面能力 — 组件、布局、主题、View DSL |
//! | [`app`] | 应用能力 — 生命周期、主循环、CLI、DI |
//! | [`data`] | 数据能力 — 配置持久化 |

// `windows::core::implement` 宏展开依赖 crate 根的 `windows_core`（无平台 cfg：依赖在各目标可用）。
extern crate windows_core;

pub mod app;
pub mod core;
pub mod data;
pub mod draw;
pub mod native;
pub mod prelude;
pub mod ui;

#[cfg(test)]
pub(crate) mod tests;
