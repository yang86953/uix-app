//! 基础设施 — 错误、几何、日志、诊断。

pub mod damage;
pub mod diagnostic;
pub mod error;
pub mod geometry;
pub mod log;
pub mod window_id;

pub use damage::*;
pub use diagnostic::*;
pub use error::*;
pub use geometry::*;
pub use log::*;
pub use window_id::*;
