//! TextBackend trait — abstract font loading, text layout, and glyph rasterization.
//!
//! The `TextBackend` trait decouples the rendering pipeline from any specific
//! font engine.  Each font format family gets its own backend:
//!
//! - `FontdueBackend` — TTF / OTF CFF1 (fontdue crate)
//! - `FreeTypeBackend` — all formats including CFF2 variable fonts (future)
//!
//! The trait is deliberately kept small: load, layout, rasterize, metrics.
//! Higher-level rendering (pixel placement, bitmap fallback) lives in the
//! `SoftwareEngine` / `RenderTarget`.

use std::sync::Arc;

use uix_platform::Error;
use crate::{FontHandle, HAlign, VAlign};

/// A single glyph positioned by text layout.
#[derive(Debug, Clone, Copy)]
pub struct PositionedGlyph {
    /// Absolute x position (top-left of glyph bounding box).
    pub x: f32,
    /// Absolute y position (top-left of glyph bounding box).
    pub y: f32,
    /// Width of the glyph's bounding box.
    pub width: f32,
    /// Height of the glyph's bounding box.
    pub height: f32,
    /// Glyph index in the font's internal glyph table.
    pub glyph_id: u32,
    /// Font handle this glyph belongs to (for multi-font layout with fallback).
    /// 默认值 FontHandle::new(u32::MAX) 表示使用调用方上下文字体。
    pub font: crate::FontHandle,
}

/// 一行文本的布局信息：包含该行所有 glyph 和行边界。
#[derive(Debug, Clone, Copy)]
pub struct LineInfo {
    /// 行在布局中的 y 坐标。
    pub y: f32,
    /// 行高度。
    pub height: f32,
    /// 行宽。
    pub width: f32,
    /// 行在源文本中的起始 byte 偏移。
    pub start_char: usize,
    /// 行在源文本中的结束 byte 偏移。
    pub end_char: usize,
    /// 该行第一个 glyph 在 glyphs 中的索引。
    pub glyph_start: usize,
    /// 该行 glyph 数量。
    pub glyph_count: usize,
}

/// Result of laying out a text string with a `TextBackend`.
#[derive(Debug, Clone)]
pub struct TextLayout {
    /// Sorted positioned glyphs (left-to-right, top-to-bottom).
    pub glyphs: Vec<PositionedGlyph>,
    /// 每行的布局信息。
    pub lines: Vec<LineInfo>,
    /// Total width of the laid-out text.
    pub width: f32,
    /// Total height of the laid-out text.
    pub height: f32,
}

/// Rasterized glyph coverage data.
#[derive(Debug, Clone)]
pub struct GlyphRaster {
    /// Width of the coverage bitmap in pixels.
    pub width: usize,
    /// Height of the coverage bitmap in pixels.
    pub height: usize,
    /// Per-pixel α coverage values (0 = transparent, 255 = opaque).
    /// Row-major order, `width * height` elements.
    pub coverage: Arc<Vec<u8>>,
    /// X bearing (left side bearing) — offset from glyph origin to bitmap
    /// left edge, in pixels. Positive = bitmap starts to the right of origin.
    pub bearing_x: f32,
    /// Y bearing (top side bearing) — offset from glyph origin to bitmap
    /// top edge, in pixels. Negative = bitmap starts above the baseline.
    pub bearing_y: f32,
}

/// Horizontal line metrics for a font at a given pixel size.
#[derive(Debug, Clone, Copy)]
pub struct LineMetrics {
    /// Ascent in pixels (above baseline).
    pub ascent: f32,
    /// Descent in pixels (below baseline, positive value).
    pub descent: f32,
    /// Full line height from baseline to baseline.
    pub new_line_size: f32,
}

/// Text layout options — mirrors the engine-level `TextLayoutOptions` but
/// stays within the text backend layer.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLayoutOptions {
    pub max_width: f32,
    pub max_height: f32,
    pub line_height: f32,
    pub word_wrap: bool,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub font_size: f32,
}

impl From<crate::TextLayoutOptions> for TextLayoutOptions {
    fn from(o: crate::TextLayoutOptions) -> Self {
        Self {
            max_width: o.max_width,
            max_height: o.max_height,
            line_height: o.line_height,
            word_wrap: o.word_wrap,
            h_align: o.h_align,
            v_align: o.v_align,
            font_size: o.font_size,
        }
    }
}

/// Abstract text rendering backend.
///
/// Each implementation owns an internal store of parsed font data.
/// `FontHandle` values (from `load_font`) act as opaque indices into that
/// store.  The backend is fully self-contained — the `SoftwareEngine` simply
/// delegates all text operations through this trait.
pub trait TextBackend: std::fmt::Debug + Send + Sync {
    /// Parse raw font bytes and store for later use.
    ///
    /// Returns an opaque `FontHandle` on success, or an error if the data
    /// cannot be parsed (e.g. unsupported format or corrupt file).
    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error>;

    /// Remove a previously loaded font, releasing its resources.
    fn unload_font(&mut self, handle: &FontHandle);

    /// Check whether a handle still refers to a valid (loaded) font.
    fn is_valid(&self, handle: &FontHandle) -> bool;

    /// Check if a font has a glyph for the given character.
    ///
    /// Uses the already-parsed font data internally — no cloning or reparsing.
    /// Called per character during multi-font text segmentation (rendering hot path).
    /// Must be efficient (O(1) cmap lookup in the font engine).
    fn has_glyph(&self, font: &FontHandle, ch: char) -> bool;

    /// Layout a text string, returning positioned glyphs.
    ///
    /// The returned `TextLayout` contains glyphs whose `x`/`y` fields are
    /// absolute positions in the layout coordinate system (origin at top-left).
    fn layout_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> TextLayout;

    /// Rasterize a single glyph at the given pixel size.
    ///
    /// Returns the coverage bitmap (α values per pixel) and dimensions.
    /// The bitmap starts at the top-left of the glyph's bounding box.
    fn rasterize_glyph(&self, font: &FontHandle, glyph_id: u32, pixel_size: f32) -> GlyphRaster;

    /// Horizontal line metrics for a font at `pixel_size`.
    fn horizontal_line_metrics(&self, font: &FontHandle, pixel_size: f32) -> Option<LineMetrics>;

    /// 返回字体的原始字节数据（供引擎检查字体覆盖范围等）。
    /// 返回 `None` 表示句柄无效或后端不支持此操作。
    fn font_data(&self, _font: &FontHandle) -> Option<Vec<u8>> {
        None
    }

    /// 设置字体回退链：当主字体缺少某个字符的字形时，依次尝试回退字体。
    ///
    /// `fallback_handles` 中的句柄必须已通过 `load_font` 加载到同一个后端。
    /// 默认实现为空操作（无回退）。
    fn set_fallback_fonts(&mut self, _fallback_handles: &[FontHandle]) {}

    /// 清空字形位图缓存（释放内存）。默认实现为空操作。
    fn clear_cache(&mut self) {}

    /// 返回后端的近似内存使用量（字节）。默认返回 0。
    fn memory_usage(&self) -> usize { 0 }
}
