//! # uix-graphics 稳定公开 API
//!
//! - [`traits`] — 行为接口（trait），各内部模块实现
//! - [`types`]  — 数据契约（值类型），跨层共享

pub mod traits;
pub mod types;

pub use traits::*;
pub use types::*;
