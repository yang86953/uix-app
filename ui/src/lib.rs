//! UIX UI — Widget framework with reactive state management.

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

pub use uix_macros::ui;
