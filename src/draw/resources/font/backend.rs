//! 文本后端契约。

use crate::core::Error;
use crate::draw::geometry::types::FontHandle;
use crate::draw::resources::font::text_backend::{
    GlyphRaster, LineMetrics, TextDirection, TextLayout, TextLayoutOptions,
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
    /// 使用 UAX #9 已解析方向布局单一方向 run；默认后端保留原布局能力。
    fn layout_text_directional(
        // 借用当前文本后端。
        &self,
        // 指定 run 使用的字体句柄。
        font: &FontHandle,
        // 指定 run 的逻辑源文本。
        text: &str,
        // 复用调用方布局约束。
        opts: &TextLayoutOptions,
        // 默认实现不消费方向，只由上层执行视觉重排。
        _direction: TextDirection,
        // 返回统一文本布局。
    ) -> TextLayout {
        // 不支持显式方向的后端仍返回可由上层重排的逻辑 cluster。
        self.layout_text(font, text, opts)
    }
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
