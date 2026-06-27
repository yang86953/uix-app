//! UIX — Rust Native UI Framework
//!
//! Modular UI framework for Windows, built in Rust.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

pub mod app;
pub mod base;
pub mod diag;
pub mod graphics;
pub mod platform;
pub mod services;
pub mod ui;

// 重新导出 proc-macro，用户可通过 `use uix::ui;` 直接使用
pub use uix_ui::ui;
