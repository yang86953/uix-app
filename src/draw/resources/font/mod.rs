//! 字体加载、文本布局与字形渲染。

pub mod backend;
pub mod bitmap_font;
pub mod font_service;
pub(crate) mod glyph_outline;
pub mod text;
pub mod text_backend;
pub mod text_backends;

pub use backend::TextBackend;
