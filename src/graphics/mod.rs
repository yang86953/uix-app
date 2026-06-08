#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
//! UIX Graphics — 2D graphics engine abstraction, Flexbox layout engine, and foundation types.
//! Layers 0+3+4: Foundation types + Graphics + Layout

pub mod bitmap_font;
pub mod color;
pub mod engine;
pub mod geometry;
pub mod layout;
pub mod null_engine;
pub mod software_engine;
pub mod types;

pub use color::*;
pub use engine::*;
pub use geometry::*;
pub use layout::*;
pub use null_engine::*;
pub use software_engine::*;
pub use types::*;
