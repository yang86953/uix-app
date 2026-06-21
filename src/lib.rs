//! UIX — Rust Native UI Framework
//!
//! Modular UI framework for Windows, built in Rust.
//! Core and Diagnostics are separate crates (`uix-core`, `uix-diag`),
//! re-exported here as `base` and `diag` for backward compatibility.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

pub use uix_core as base;
pub use uix_diag as diag;

pub mod app;
pub mod graphics;
pub mod platform;
pub mod services;
pub mod ui;

// 重新导出 proc-macro，用户可通过 `use uix::ui;` 直接使用
pub use uix_macros::ui;
