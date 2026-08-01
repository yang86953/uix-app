//! 文本后端契约。

use crate::core::Error;
use crate::draw::geometry::types::FontHandle;
use crate::draw::resources::font::text_backend::{
    GlyphRaster, LineMetrics, TextLayout, TextLayoutOptions,
};

/// 字体文本后端 — 字体加载、布局与 glyph 光栅化。
pub trait TextBackend: std::fmt::Debug + Send + Sync {
    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error>;
    /// 消费所有权的字体加载：内部路径（fs::read 已持有 Vec）避免二次复制。
    fn load_font_owned(&mut self, data: Vec<u8>) -> Result<FontHandle, Error> {
        self.load_font(&data)
    }
    /// 内存映射字体加载：字体文件按需分页，未触达字形不驻留 working set。
    /// 默认实现退化为拷贝（后端不支持映射时保底正确）。
    fn load_font_mapped(&mut self, mmap: memmap2::Mmap) -> Result<FontHandle, Error> {
        self.load_font(mmap.as_ref())
    }
    fn unload_font(&mut self, handle: &FontHandle);
    fn is_valid(&self, handle: &FontHandle) -> bool;
    fn has_glyph(&self, font: &FontHandle, ch: char) -> bool;
    fn layout_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> TextLayout;
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
