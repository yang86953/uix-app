#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

//! UIX UI — Widget framework with reactive state management.
//! Layer 5: Declarative widget tree with 10-manager system architecture.

pub mod animation;
pub mod children;
pub mod context;
pub mod layout;
pub mod macros;
pub mod managers;
pub mod render_context;
pub mod state;
pub mod style;
pub mod theme;
pub mod widget;
pub mod widgets;

mod api;
pub use api::*;

// 重新导出 proc-macro `ui!`
pub use uix_macros::ui;
