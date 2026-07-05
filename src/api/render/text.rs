//! 文本协议 — 字体布局、glyph 光栅化与文本后端。

pub use crate::render::bitmap_font::BitmapFont;
pub use crate::render::font_service::FontService;
pub use crate::render::text_backend::{GlyphRaster, LineInfo, LineMetrics, PositionedGlyph, TextLayout};
pub use crate::render::types::TextLayoutOptions;

use super::primitives::FontHandle;
use crate::render::text_backend::TextLayoutOptions as LayoutOpts;
use crate::platform::Error;

/// 字体文本后端 — 字体加载、布局与 glyph 光栅化。
pub trait TextBackend: std::fmt::Debug + Send + Sync {
    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error>;
    fn unload_font(&mut self, handle: &FontHandle);
    fn is_valid(&self, handle: &FontHandle) -> bool;
    fn has_glyph(&self, font: &FontHandle, ch: char) -> bool;
    fn layout_text(&self, font: &FontHandle, text: &str, opts: &LayoutOpts) -> TextLayout;
    fn rasterize_glyph(&self, font: &FontHandle, glyph_id: u32, pixel_size: f32) -> GlyphRaster;
    fn horizontal_line_metrics(&self, font: &FontHandle, pixel_size: f32) -> Option<LineMetrics>;

    fn font_data(&self, _font: &FontHandle) -> Option<Vec<u8>> {
        None
    }
    fn set_fallback_fonts(&mut self, _fallback_handles: &[FontHandle]) {}
    fn clear_cache(&mut self) {}
    fn memory_usage(&self) -> usize {
        0
    }
}
