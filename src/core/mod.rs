//! 基础设施 — 错误、几何。

pub mod component_id;
pub mod damage;
pub mod error;
pub mod geometry;
pub(crate) mod glyph_outline;
pub mod perf_probe;
pub mod window_id;

pub use component_id::*;
pub use damage::*;
pub use error::*;
pub use geometry::*;
pub use window_id::*;
