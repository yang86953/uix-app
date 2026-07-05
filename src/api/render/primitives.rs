//! 基础图形类型 — 颜色、路径、描边与混合模式。

pub use crate::render::color::{colors, Color};
pub use crate::render::path::{FillRule, LineCap, LineJoin, Path, PathBuilder, PathSegment};
pub use crate::render::stroker::StrokeOptions;
pub use crate::render::types::{
    BlendMode, DirtyRegion, FontHandle, GradientDirection, HAlign, ImageHandle, Radius,
    TextLayoutOptions, Transform, VAlign,
};
