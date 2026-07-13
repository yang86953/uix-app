//! 平台能力 — OS 抽象，差异封装在此域。

pub(crate) mod backends;
pub mod factory;
pub(crate) mod graphics;
pub mod presenter;
pub mod services;
pub mod shared;
pub mod test_harness;
pub mod traits;

pub use crate::core::damage::*;
pub use crate::core::error::*;
pub use factory::{available_memory_bytes, create_platform};
pub use services::file_service;
pub use services::notification;
