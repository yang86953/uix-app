//! UIX — Rust Native UI Framework
//!
//! Modular UI framework for Windows, built in Rust.
//! Foundation types (geometry, error, status, input) are in `uix-platform`.
//! 服务层已合并至 platform crate（见 uix_platform::file_service / notification / settings）。
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

pub use uix_app as app;
pub use uix_graphics as graphics;
pub use uix_platform as platform;
pub use uix_ui as ui;

// 重新导出 macros
pub use uix_ui::ui;
pub use uix_ui::define_widget;
pub use uix_ui::tree;

// ── 简化 API（用户可通过 `use uix::view::*` 使用）──
pub use uix_ui::view;
pub use uix_ui::App;
pub use uix_ui::StyleExt;
pub use uix_ui::Ui;
pub use uix_ui::View;
pub use uix_ui::ViewNode;
