//! UIX App — Application entry point and window lifecycle.

pub mod application;
pub mod cli;
pub mod di;
pub mod window;

pub use crate::application::{map_ui_event, App, AppMode};
pub use crate::cli::Cli;
pub use crate::di::Container;
pub use crate::window::Window;
