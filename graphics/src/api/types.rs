//! # uix-graphics 数据契约
//!
//! 本模块定义 graphics 层的共享数据结构（值类型）。

// ── 基础图形类型 ──
pub use crate::color::{colors, Color};
pub use crate::types::{
    BlendMode, DirtyRegion, FontHandle, GradientDirection, HAlign, ImageHandle, Radius,
    TextLayoutOptions, Transform, VAlign,
};

// ── 帧渲染结果 ──
pub use crate::engine::RenderOutcome;

// ── 路径系统 ──
pub use crate::path::{FillRule, LineCap, LineJoin, Path, PathBuilder, PathSegment};

// ── 描边 ──
pub use crate::stroker::StrokeOptions;

// ── 字体文本 ──
pub use crate::text_backend::{GlyphRaster, LineInfo, LineMetrics, PositionedGlyph, TextLayout};

// ── 空间坐标系统 ──
pub use crate::spatial::unit::AngleExt;
pub use crate::spatial::{
    DirtyRegion3D, IntoAABB3D, Mat4, Orientation, PhysicalBox, PhysicalUnit, PhysicalUnitExt,
    Quad2D, Ray3D, SpatialContext, Vec2, Vec3, Vec4, AABB3D,
};

// ── 引擎实现 ──
pub use crate::backend::{BackendCapabilities, BackendKind, DamageRegion, RenderBackend};
pub use crate::engine::cpu::software::SoftwareEngine;
pub use crate::gpu_engine::GpuEngine;
pub use crate::null_engine::NullEngine;
pub use crate::pipeline::{
    AnimationRegistry, Invalidation, InvalidationQueue, InvalidationSource, NodeId, RenderMetrics,
    RenderSession, ScrollDelta,
};

// ── 其他 ──
pub use crate::bitmap_font::BitmapFont;
pub use crate::font_service::FontService;
