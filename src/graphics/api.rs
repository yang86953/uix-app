// ============================================================================
// graphics/api.rs — Graphics 层的公共 API 出口
//
// 本文件定义 graphics 层对外暴露的公共接口。其他层只能通过本文件
// 使用 graphics 层的功能，禁止直接引用内部模块。
// ============================================================================

// ── 基础图形类型 ──
pub use crate::graphics::color::{Color, colors};
pub use crate::graphics::types::{
    BlendMode, DirtyRegion, FontHandle, GradientDirection, HAlign, ImageHandle,
    Radius, TextLayoutOptions, Transform, VAlign,
};

// ── 渲染引擎 Trait ──
pub use crate::graphics::engine::{GraphicsEngine, RenderOutcome};

// ── 空实现引擎 ──
pub use crate::graphics::null_engine::NullEngine;

// ── SoftwareEngine ──
pub use crate::graphics::software_engine::SoftwareEngine;

// ── Flexbox 布局 ──
pub use crate::graphics::layout::{
    AlignItems, FlexChild, FlexDirection, FlexInput, FlexOutput, JustifyContent,
    compute_flex_layout,
};

// ── Grid 布局 ──
pub use crate::graphics::layout::{
    GridChild, GridInput, GridOutput, GridTrack, compute_grid_layout,
};

// ── 字体文本后端（仅 trait 和布局类型） ──
pub use crate::graphics::text_backend::{
    GlyphRaster, LineMetrics, PositionedGlyph, TextBackend, TextLayout,
};

// ── Bitmap 字体 ──
pub use crate::graphics::bitmap_font::BitmapFont;

// ── 路径系统 ──
pub use crate::graphics::path::{FillRule, LineCap, LineJoin, Path, PathBuilder, PathSegment};
pub use crate::graphics::rasterizer::fill_polygons;
pub use crate::graphics::stroker::{stroke_path, StrokeOptions};

// ── 帧图（Frame Graph） ──
pub use crate::graphics::frame_graph::FrameGraph;
pub use crate::graphics::frame_graph::compile::{CompiledGraph, Compiler};
pub use crate::graphics::frame_graph::pass::{FrameResources, PassBuilder, PassContext, PassNode};
pub use crate::graphics::frame_graph::resource::{PassId, ResourceRegistry};

