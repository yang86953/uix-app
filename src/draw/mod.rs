//! 渲染系统 — 2D 引擎、光栅化、字体与合成管线。

pub mod api;
pub mod backend;
pub mod command;
pub mod debug;
pub mod geometry;
pub mod renderer;
pub mod resources;
pub mod scene;

pub use crate::core::{DamageRegion, DirtyRegion};
pub use api::Canvas2D;
pub use backend::{BackendCapabilities, BackendKind, RenderBackend};
pub use geometry::color::{colors, Color};
pub use geometry::path::{FillRule, LineCap, LineJoin, Path, PathBuilder, PathSegment};
pub use geometry::spatial::unit::AngleExt;
pub use geometry::spatial::{
    IntoAABB3D, Mat4, Orientation, PhysicalBox, PhysicalUnit, PhysicalUnitExt, Quad2D, Ray3D,
    SpatialContext, Vec2, Vec3, Vec4, AABB3D,
};
pub use geometry::stroker::StrokeOptions;
pub use geometry::types::{
    BlendMode, FontHandle, GradientDirection, HAlign, ImageHandle, Radius, TextLayoutOptions,
    Transform, VAlign,
};
pub use geometry::{color, flattener, path, stroker, tessellator, types};
pub use renderer::Renderer;
pub use renderer::{
    AnimationRegistry, Invalidation, InvalidationQueue, InvalidationSource, NodeId, RenderMetrics,
    RenderOutcome, ScrollDelta,
};
pub use renderer::{
    GraphicsCapabilities, PresentationMode, RasterPipeline, RenderTarget, ScrollCopy,
    UpdateStrategy,
};
pub use resources::font::TextBackend;
pub use resources::{
    BitmapFont, BitmapHandle, FontService, GlyphRaster, ImageService, ImageSlot, LineInfo,
    LineMetrics, PositionedGlyph, TextLayout,
};
