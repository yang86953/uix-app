//! 渲染系统 — 2D 引擎、光栅化、字体与合成管线。

pub mod backend;
pub mod compositor;
pub mod debug;
pub mod engine;
pub mod font;
pub mod gpu_engine;
pub mod null_engine;
pub mod painting;
pub mod pipeline;
pub mod primitives;
pub mod rasterizer;
pub mod render_object;
pub mod spatial;
pub mod traits;

// ── 兼容旧 graphics 子模块路径 ────────────────────────────────
pub use font::{bitmap_font, font_service, text, text_backend, text_backends};
pub use primitives::{blur, color, flattener, path, stroker, types};

// 公开契约重导出
pub use crate::api::render::*;
