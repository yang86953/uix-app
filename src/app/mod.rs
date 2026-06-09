#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
//! UIX Application — Entry point for GUI and CLI applications.
//! Layer 6: App lifecycle, Window event loop, CLI interface.

pub mod application;
pub mod cli;
pub mod di;
pub mod window;

pub use application::*;
pub use cli::*;
pub use di::*;
pub use window::*;
