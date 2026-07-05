#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

//! UIX — Rust Native UI Framework
//!
//! # 模块划分
//!
//! | 模块 | 职责 |
//! |------|------|
//! | [`platform`] | 平台系统 — OS 抽象（Win32 / Wayland） |
//! | [`render`] | 渲染系统 — 2D 引擎、光栅化、字体、合成 |
//! | [`widget`] | 组件系统 — Widget 框架、布局、主题、内置组件 |
//! | [`runtime`] | 运行时系统 — 应用入口、窗口、CLI、DI |
//! | [`view`] | 视图系统 — 声明式 UI API |
//! | [`api`] | 各系统稳定公开契约 |

pub mod api;
pub mod platform;
pub mod render;
pub mod runtime;
pub mod view;
pub mod widget;
