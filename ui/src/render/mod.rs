//! 渲染管线：事件循环与 graphics 桥接。

mod bridges;
pub mod event_loop;
mod scene_paint;
mod style_ext;

pub use event_loop::*;
pub use style_ext::{apply_style, RenderContextStyleExt};

pub use uix_graphics::compositor::{LayerNode, LayerTree, ScenePaint};
pub use uix_graphics::debug::DebugRenderService;
pub use uix_graphics::painting::{
    PaintContext, RenderContext, ThemeSnapshot, ThemeTokens, resolve_font_size,
};
pub use uix_graphics::pipeline::{FrameRenderInput, FrameRenderOutput, FrameRenderer};
pub use uix_graphics::text::TextRenderService;
