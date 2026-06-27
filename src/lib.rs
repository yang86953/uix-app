//! UIX — Rust Native UI Framework
//!
//! Modular UI framework for Windows, built in Rust.
//! Foundation types (geometry, error, status, input) are in `uix-platform`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

pub use uix_services as diag;

pub use uix_app as app;
pub use uix_graphics as graphics;
pub use uix_platform as platform;
pub use uix_services as services;
pub use uix_ui as ui;

// 重新导出 macros
pub use uix_ui::ui;
pub use uix_ui::define_widget;
pub use uix_ui::tree;
