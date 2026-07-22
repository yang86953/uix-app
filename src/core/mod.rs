//! 基础设施 — 错误、几何、日志、诊断。

pub mod component_id;
pub mod damage;
pub mod diagnostic;
pub mod error;
pub mod geometry;
pub(crate) mod glyph_outline;
pub mod log;
pub mod perf_probe;
pub mod window_id;

pub use component_id::*;
pub use damage::*;
pub use diagnostic::*;
pub use error::*;
pub use geometry::*;
pub use log::*;
pub use window_id::*;
