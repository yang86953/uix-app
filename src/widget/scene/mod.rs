//! 渲染管线：事件循环与 graphics 桥接。

mod bridges;
pub mod event_loop;
mod scene_paint;
mod style_ext;

pub use event_loop::*;
pub use style_ext::{apply_style, RenderContextStyleExt};

pub use crate::render::compositor::{LayerNode, LayerTree, ScenePaint};
pub use crate::render::debug::DebugRenderService;
pub use crate::render::painting::{
    PaintContext, RenderContext, ThemeSnapshot, ThemeTokens, resolve_font_size,
};
pub use crate::render::pipeline::{FrameRenderInput, FrameRenderOutput, FrameRenderer};
pub use crate::render::font::text::TextRenderService;
