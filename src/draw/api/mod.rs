//! 绘制上下文。

pub mod canvas;
pub mod paint_context;

pub use canvas::Canvas2D;
pub use paint_context::{resolve_font_size, PaintContext, PaintSurfaceConfig};
