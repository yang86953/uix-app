#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
//! UIX Graphics — 2D graphics engine abstraction and foundation types.
//!
//! # 稳定公开 API
//!
//! 建议优先使用 [`api`] 模块导入公开类型：
//! ```ignore
//! use uix_graphics::api::{Color, Path, Vec2, GraphicsEngine, FrameGraph};
//! ```

pub mod api;
pub mod bitmap_font;
pub mod blur;
pub mod color;
pub mod engine;
pub mod flattener;
pub mod font_service;
pub mod frame_graph;
pub mod gpu_engine;
pub mod null_engine;
pub mod path;
pub mod rasterizer;
pub mod spatial;
pub mod stroker;
pub mod text_backend;
pub mod text_backends;
pub mod traits;
pub mod types;

pub use api::*;
