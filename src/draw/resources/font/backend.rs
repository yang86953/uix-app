//! 文本后端契约。

// 使用共享只读字体字节，避免随包 CJK 字体在组合根交接时重复复制。
use std::sync::Arc;

use crate::core::Error;
use crate::draw::geometry::types::FontHandle;
use crate::draw::resources::font::text_backend::{
    GlyphRaster, LineMetrics, TextDirection, TextLayout, TextLayoutOptions,
};

/// 字体文本后端 — 字体加载、布局与 glyph 光栅化。
pub trait TextBackend: std::fmt::Debug + Send + Sync {
    /// 从借用字节加载字体并返回后端句柄。
    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error>;
    /// 消费所有权的字体加载：内部路径（fs::read 已持有 Vec）避免二次复制。
    fn load_font_owned(&mut self, data: Vec<u8>) -> Result<FontHandle, Error> {
        self.load_font(&data)
    }
    /// 从共享只读字节加载字体；默认后端可沿用借用加载语义。
    fn load_font_shared(&mut self, data: Arc<[u8]>) -> Result<FontHandle, Error> {
        // 默认实现保持第三方文本后端兼容，专用后端可覆盖以取得零复制所有权。
        self.load_font(data.as_ref())
    }
    /// 从进程期静态字节加载字体；默认后端保留借用加载兼容语义。
    fn load_font_static(&mut self, data: &'static [u8]) -> Result<FontHandle, Error> {
        // 第三方后端无需理解静态所有权，仍可沿用既有复制实现。
        self.load_font(data)
    }
    /// 内存映射字体加载：字体文件按需分页，未触达字形不驻留 working set。
    /// 默认实现退化为拷贝（后端不支持映射时保底正确）。
    fn load_font_mapped(&mut self, mmap: memmap2::Mmap) -> Result<FontHandle, Error> {
        self.load_font(mmap.as_ref())
    }
    /// 卸载字体句柄及其后端资源。
    fn unload_font(&mut self, handle: &FontHandle);
    /// 返回字体句柄当前是否仍由该后端持有。
    fn is_valid(&self, handle: &FontHandle) -> bool;
    /// 返回字体是否覆盖指定字符。
    fn has_glyph(&self, font: &FontHandle, ch: char) -> bool;
    /// 按布局选项塑形并排列文本。
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
    /// 将指定字形按像素尺寸光栅化。
    fn rasterize_glyph(&self, font: &FontHandle, glyph_id: u32, pixel_size: f32) -> GlyphRaster;
    /// 返回指定像素尺寸下的水平行指标。
    fn horizontal_line_metrics(&self, font: &FontHandle, pixel_size: f32) -> Option<LineMetrics>;

    /// 返回字体源数据的副本；后端不保留数据时返回 `None`。
    fn font_data(&self, _font: &FontHandle) -> Option<Vec<u8>> {
        None
    }
    /// 设置按优先顺序参与缺字回退的字体句柄。
    fn set_fallback_fonts(&mut self, _fallback_handles: &[FontHandle]) {}
    /// 清除后端持有的可重建缓存。
    fn clear_cache(&mut self) {}
    /// 返回后端当前估算的字体资源内存字节数。
    fn memory_usage(&self) -> usize {
        0
    }
}
