//! 字体文本后端类型定义（TextBackend trait 已迁移至 crate::api::text）。

/// A single glyph positioned by text layout.
#[derive(Debug, Clone, Copy)]
pub struct PositionedGlyph {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub glyph_id: u32,
    pub font: crate::api::render::FontHandle,
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
    pub coverage: std::sync::Arc<Vec<u8>>,
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
    pub h_align: crate::api::render::HAlign,
    pub v_align: crate::api::render::VAlign,
    pub font_size: f32,
}

impl From<crate::api::render::TextLayoutOptions> for TextLayoutOptions {
    fn from(o: crate::api::render::TextLayoutOptions) -> Self {
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
