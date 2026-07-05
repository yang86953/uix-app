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

pub use font::{bitmap_font, font_service, text, text_backend, text_backends};
pub use primitives::{blur, color, flattener, path, stroker, types};
pub use primitives::color::{colors, Color};
pub use primitives::path::{FillRule, LineCap, LineJoin, Path, PathBuilder, PathSegment};
pub use primitives::stroker::StrokeOptions;
pub use primitives::types::{
    BlendMode, DirtyRegion, FontHandle, GradientDirection, HAlign, ImageHandle, Radius,
    TextLayoutOptions, Transform, VAlign,
};
pub use engine::RenderOutcome;
pub use engine::cpu::software::SoftwareEngine;
pub use gpu_engine::GpuEngine;
pub use null_engine::NullEngine;
pub use pipeline::{
    AnimationRegistry, Invalidation, InvalidationQueue, InvalidationSource, NodeId, RenderMetrics,
    RenderSession, ScrollDelta,
};
pub use backend::{BackendCapabilities, BackendKind, DamageRegion, RenderBackend};
pub use font::bitmap_font::BitmapFont;
pub use font::font_service::FontService;
pub use font::text_backend::{GlyphRaster, LineInfo, LineMetrics, PositionedGlyph, TextLayout};
pub use spatial::unit::AngleExt;
pub use spatial::{
    DirtyRegion3D, IntoAABB3D, Mat4, Orientation, PhysicalBox, PhysicalUnit, PhysicalUnitExt,
    Quad2D, Ray3D, SpatialContext, Vec2, Vec3, Vec4, AABB3D,
};
pub use traits::{
    Canvas2D, GraphicsCapabilities, GraphicsEngine, PresentationMode, RenderingBackend,
    TextBackend, UpdateStrategy,
};
