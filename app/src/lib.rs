//! UIX App — Application entry point and window lifecycle.
//!
//! # 稳定公开 API
//!
//! 建议优先使用 [`api`] 模块导入公开类型：
//! ```ignore
//! use uix_app::api::{App, Cli, Container, Window};
//! ```

pub mod api;
pub mod application;
pub mod cli;
pub mod di;
pub mod window;

pub use api::*;
