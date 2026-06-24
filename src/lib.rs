//! UIX — Rust Native UI Framework
//!
//! Modular UI framework for Windows, built in Rust.
//! Core and Diagnostics are separate crates (`uix-core`, `uix-diag`),
//! re-exported here as `base` and `diag` for backward compatibility.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

pub use uix_core as base;
pub use uix_diag as diag;

pub use uix_app as app;
pub use uix_graphics as graphics;
pub use uix_platform as platform;
pub use uix_services as services;
pub use uix_ui as ui;

// 重新导出 macros
pub use uix_macros::ui;
pub use uix_ui::define_widget;
pub use uix_ui::tree;
