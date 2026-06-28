#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
//! UIX Graphics — 2D graphics engine abstraction and foundation types.

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

// ── 空间坐标系统 ──
pub use crate::spatial::{
    AABB3D, DirtyRegion3D, IntoAABB3D, Mat4, Orientation, PhysicalBox, PhysicalUnit,
    PhysicalUnitExt, Quad2D, Ray3D, SpatialContext, Vec2, Vec3, Vec4,
};
pub use crate::spatial::unit::AngleExt;

// ── 基础图形类型 ──
pub use crate::color::{colors, Color};
pub use crate::types::{
    BlendMode, DirtyRegion, FontHandle, GradientDirection, HAlign, ImageHandle, Radius,
    TextLayoutOptions, Transform, VAlign,
};

// ── 渲染引擎 ──
pub use crate::traits::GraphicsEngine;
pub use crate::engine::RenderOutcome;
pub use crate::traits::{Canvas2D, RenderingBackend, UpdateStrategy};

// ── 引擎实现 ──
pub use crate::null_engine::NullEngine;
pub use crate::gpu_engine::GpuEngine;
pub use crate::engine::cpu::software::SoftwareEngine;

// ── 字体文本后端 ──
pub use crate::text_backend::{
    GlyphRaster, LineMetrics, PositionedGlyph, TextBackend, TextLayout,
};

// ── Bitmap 字体 ──
pub use crate::bitmap_font::BitmapFont;

// ── 路径系统 ──
pub use crate::path::{FillRule, LineCap, LineJoin, Path, PathBuilder, PathSegment};
pub use crate::rasterizer::fill_polygons;
pub use crate::stroker::{stroke_path, StrokeOptions};

// ── 帧图 ──
pub use crate::frame_graph::compile::{CompiledGraph, Compiler};
pub use crate::frame_graph::pass::{FrameResources, PassBuilder, PassContext, PassNode};
pub use crate::frame_graph::resource::{PassId, ResourceRegistry};
pub use crate::frame_graph::FrameGraph;
