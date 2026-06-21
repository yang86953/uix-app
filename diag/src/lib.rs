//! UIX Diagnostics — Error types, structured logging, error collection, and recovery policies.

pub mod collector;
pub mod error;
pub mod fatal;
pub mod log;
pub mod log_format;
pub mod recovery;
pub mod recovery_policy;
pub mod result;

mod api;
pub use api::*;
