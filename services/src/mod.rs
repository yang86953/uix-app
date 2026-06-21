#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
//! UIX Services — Business service layer.
//! Settings persistence, file I/O, and middleware pipeline.

pub mod file_service;
pub mod middleware;
pub mod notification_service;
pub mod settings;

mod api;
pub use api::*;
