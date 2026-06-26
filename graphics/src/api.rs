// ============================================================================
// graphics/api.rs — Graphics 层的公共 API 出口
//
// 本文件定义 graphics 层对外暴露的公共接口。其他层只能通过本文件
// 使用 graphics 层的功能，禁止直接引用内部模块。
// ============================================================================

// ── 基础图形类型 ──
pub use crate::color::{colors, Color};
pub use crate::types::{
    BlendMode, DirtyRegion, FontHandle, GradientDirection, HAlign, ImageHandle, Radius,
    TextLayoutOptions, Transform, VAlign,
};

// ── 渲染引擎 Trait ──
pub use crate::traits::GraphicsEngine;
pub use crate::engine::RenderOutcome;

// ── 渲染策略 ──
pub use crate::traits::{Canvas2D, Canvas3D, RenderingBackend, UpdateStrategy};

// ── 空实现引擎 ──
pub use crate::null_engine::NullEngine;

// ── GPU 引擎 ──
pub use crate::gpu_engine::GpuEngine;

// ── SoftwareEngine ──
pub use crate::engine::cpu::software::SoftwareEngine;

// 布局引擎（Flexbox + Grid）已迁移至 ui::layout
// FlexDirection, AlignItems, JustifyContent 等类型及计算函数改从 ui::layout 导入

// ── 字体文本后端（仅 trait 和布局类型） ──
pub use crate::text_backend::{
    GlyphRaster, LineMetrics, PositionedGlyph, TextBackend, TextLayout,
};

// ── Bitmap 字体 ──
pub use crate::bitmap_font::BitmapFont;

// ── 路径系统 ──
pub use crate::path::{FillRule, LineCap, LineJoin, Path, PathBuilder, PathSegment};
pub use crate::rasterizer::fill_polygons;
pub use crate::stroker::{stroke_path, StrokeOptions};

// ── 帧图（Frame Graph） ──
pub use crate::frame_graph::compile::{CompiledGraph, Compiler};
pub use crate::frame_graph::pass::{FrameResources, PassBuilder, PassContext, PassNode};
pub use crate::frame_graph::resource::{PassId, ResourceRegistry};
pub use crate::frame_graph::FrameGraph;
