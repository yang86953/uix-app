//! 平台能力 — OS 抽象，差异封装在此域。

pub mod backends;
pub mod factory;
pub mod presenter;
pub mod services;
pub mod shared;
pub mod test_harness;
pub mod traits;

pub use crate::core::error::*;
pub use crate::core::geometry::*;
pub use factory::{available_memory_bytes, create_gpu_context, create_platform};
pub use services::file_service;
pub use services::notification;
pub use traits::*;
