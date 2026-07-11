//! 字体文本后端类型定义（TextBackend trait 已迁移至 `crate::draw::traits::text`）。

/// 缺字占位（tofu）字形 ID。后端无真实轮廓时由 `FontService::rasterize_glyph` 合成方框。
pub const TOFU_GLYPH_ID: u32 = u32::MAX - 1;

/// A single glyph positioned by text layout.
#[derive(Debug, Clone, Copy)]
pub struct PositionedGlyph {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub glyph_id: u32,
    /// 对应源文本中的 Unicode 标量下标（`chars()` 序），与 glyph 下标解耦。
    pub char_index: usize,
    pub font: crate::draw::FontHandle,
}

/// 一行文本的布局信息。
#[derive(Debug, Clone, Copy)]
pub struct LineInfo {
    pub y: f32,
    pub height: f32,
    pub width: f32,
    pub start_char: usize,
    pub end_char: usize,
    pub glyph_start: usize,
    pub glyph_count: usize,
}

/// 文本布局结果。
#[derive(Debug, Clone)]
pub struct TextLayout {
    pub glyphs: Vec<PositionedGlyph>,
    pub lines: Vec<LineInfo>,
    pub width: f32,
    pub height: f32,
}

/// 光栅化字形数据。
#[derive(Debug, Clone)]
pub struct GlyphRaster {
    pub width: usize,
    pub height: usize,
    pub coverage: std::sync::Arc<[u8]>,
    pub bearing_x: f32,
    pub bearing_y: f32,
}

/// 字体水平度量。
#[derive(Debug, Clone, Copy)]
pub struct LineMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub new_line_size: f32,
}

/// Text layout options.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLayoutOptions {
    pub max_width: f32,
    pub max_height: f32,
    pub line_height: f32,
    pub word_wrap: bool,
    pub h_align: crate::draw::HAlign,
    pub v_align: crate::draw::VAlign,
    pub font_size: f32,
}

impl From<crate::draw::TextLayoutOptions> for TextLayoutOptions {
    fn from(o: crate::draw::TextLayoutOptions) -> Self {
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
