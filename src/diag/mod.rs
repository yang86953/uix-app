#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
//! UIX Diagnostics — Error types, structured logging, error collection, and recovery policies.
//! Provides the full diagnostics stack used by all UIX crates.

pub mod collector;
pub mod error;
pub mod fatal;
pub mod log;
pub mod recovery;
pub mod result;

pub use collector::*;
pub use error::*;
pub use fatal::*;
pub use log::*;
pub use recovery::*;
pub use result::*;
