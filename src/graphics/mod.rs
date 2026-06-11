#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
//! UIX Graphics — 2D graphics engine abstraction, Flexbox layout engine, and foundation types.
//! Layers 0+3+4: Foundation types + Graphics + Layout

pub mod bitmap_font;
pub mod color;
pub mod engine;
pub mod frame_graph;
pub mod layout;
pub mod null_engine;
pub mod software_engine;
pub mod text_backend;
pub mod types;

mod api;
pub use api::*;