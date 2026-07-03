#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
//! UIX Graphics — 2D graphics engine abstraction and foundation types.
//!
//! # 稳定公开 API
//!
//! 建议优先使用 [`api`] 模块导入公开类型：
//! ```ignore
//! use uix_graphics::api::{Color, Path, RenderSession, SoftwareEngine};
//! ```

pub mod api;
pub mod backend;
pub mod bitmap_font;
pub mod blur;
pub mod color;
pub mod compositor;
pub mod debug;
pub mod engine;
pub mod flattener;
pub mod font_service;
pub mod gpu_engine;
pub mod null_engine;
pub mod painting;
pub mod path;
pub mod pipeline;
pub mod rasterizer;
pub mod render_object;
pub mod spatial;
pub mod stroker;
pub mod text;
pub mod text_backend;
pub mod text_backends;
pub mod traits;
pub mod types;

pub use api::*;
