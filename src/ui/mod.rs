#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]

//! UIX UI — Widget framework with reactive state management.
//! Layer 5: Declarative widget tree with 10-manager system architecture.

pub mod animation;
pub mod children;
pub mod context;
pub mod macros;
pub mod managers;
pub mod render_context;
pub mod state;
pub mod style;
pub mod theme;
pub mod widget;
pub mod widget_builder;
pub mod widgets;

pub use animation::*;
pub use children::*;
pub use context::*;
pub use managers::*;
pub use render_context::*;
pub use state::*;
pub use style::*;
pub use theme::*;
pub use widget::*;
pub use widget_builder::*;
pub use widgets::*;
