//! 字体文本后端类型定义（TextBackend trait 已迁移至 `crate::draw::traits::text`）。

/// 缺字占位（tofu）字形 ID。后端无真实轮廓时由 `FontService::rasterize_glyph` 合成方框。
pub const TOFU_GLYPH_ID: u32 = u32::MAX - 1;

pub(crate) fn prohibited_at_line_start(ch: char) -> bool {
    matches!(
        ch,
        '，' | '。'
            | '、'
            | '；'
            | '：'
            | '！'
            | '？'
            | '）'
            | '】'
            | '》'
            | '〉'
            | '〕'
            | '］'
            | '｝'
            | '”'
            | '’'
            | '…'
            | '—'
            | ','
            | '.'
            | ';'
            | ':'
            | '!'
            | '?'
            | ')'
            | ']'
            | '}'
    )
}

fn is_wide_scalar(ch: char) -> bool {
    matches!(ch,
        '\u{1100}'..='\u{11ff}'
        | '\u{2e80}'..='\u{a4cf}'
        | '\u{ac00}'..='\u{d7af}'
        | '\u{f900}'..='\u{faff}'
        | '\u{fe10}'..='\u{fe6f}'
        | '\u{ff00}'..='\u{ffef}'
        | '\u{1f000}'..='\u{1faff}'
        | '\u{20000}'..='\u{3ffff}'
    )
}

fn estimated_scalar_width(ch: char, font_size: f32) -> f32 {
    let factor = match ch {
        '\n' | '\r' => 0.0,
        ' ' => 0.35,
        '\t' => 2.0,
        'm' | 'M' | 'W' | 'w' => 0.7,
        'i' | 'I' | 'l' | '1' | '.' | ',' | ':' | ';' | '\'' => 0.3,
        c if is_wide_scalar(c) => 1.0,
        _ => 0.55,
    };
    font_size * factor
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct EstimatedTextMetrics {
    pub max_line_width: f32,
    pub line_count: usize,
    pub width_wrapped: bool,
}

/// 无字体上下文时使用的文本尺寸估算；显式换行与 CJK 行首禁则须与真实布局一致。
pub(crate) fn estimate_text_metrics(
    text: &str,
    max_width: f32,
    font_size: f32,
) -> EstimatedTextMetrics {
    let mut line_width = 0.0f32;
    let mut widest_line = 0.0f32;
    let mut line_count = 1usize;
    let mut width_wrapped = false;
    let mut line_char_count = 0usize;
    let mut last_width = 0.0f32;
    let wraps = max_width.is_finite() && max_width > 0.0;

    for ch in text.chars() {
        if ch == '\n' {
            widest_line = widest_line.max(line_width);
            line_width = 0.0;
            line_count += 1;
            line_char_count = 0;
            last_width = 0.0;
            continue;
        }
        if ch == '\r' {
            continue;
        }
        let width = estimated_scalar_width(ch, font_size);
        if wraps && line_width > 0.0 && line_width + width > max_width {
            if prohibited_at_line_start(ch) && line_char_count > 1 {
                line_width -= last_width;
                widest_line = widest_line.max(line_width);
                line_width = last_width + width;
                line_char_count = 2;
                last_width = width;
                line_count += 1;
                width_wrapped = true;
            } else if prohibited_at_line_start(ch) {
                line_width += width;
                line_char_count += 1;
                last_width = width;
            } else {
                widest_line = widest_line.max(line_width);
                line_width = width;
                line_char_count = 1;
                last_width = width;
                line_count += 1;
                width_wrapped = true;
            }
        } else {
            line_width += width;
            line_char_count += 1;
            last_width = width;
        }
    }

    EstimatedTextMetrics {
        max_line_width: widest_line.max(line_width),
        line_count,
        width_wrapped,
    }
}

/// 单个字形的最大光栅化字号，限制异常输入导致的面积型内存增长。
pub(crate) const MAX_RASTER_PIXEL_SIZE: f32 = 512.0;

pub(crate) fn bounded_font_size(pixel_size: f32) -> f32 {
    if pixel_size.is_finite() {
        pixel_size.clamp(1.0, MAX_RASTER_PIXEL_SIZE)
    } else {
        1.0
    }
}

pub(crate) fn normalized_raster_pixel_size(pixel_size: f32) -> Option<u32> {
    if !pixel_size.is_finite() || pixel_size <= 0.0 {
        return None;
    }
    Some(pixel_size.round().clamp(1.0, MAX_RASTER_PIXEL_SIZE) as u32)
}

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
///
/// `outline_mesh` 为 NonZero 边列表 `[ax,ay,bx,by,…]`（本地像素，含 MSDF 范围 fringe）；
/// 同时缓存字体光栅器的面积 `coverage` 供物理 1:1 R8 路径复用。严格 GPU
/// 缩放、仿射或高 DPR 走 RGBA8 MSDF。
#[derive(Debug, Clone)]
pub struct GlyphRaster {
    pub width: usize,
    pub height: usize,
    pub coverage: std::sync::Arc<[u8]>,
    pub bearing_x: f32,
    pub bearing_y: f32,
    /// GPU atlas 覆盖边列表；物理 1:1 时使用 `coverage`，缩放、仿射或高 DPR 走 MSDF。
    pub outline_mesh: Option<std::sync::Arc<[f32]>>,
}

impl GlyphRaster {
    pub(crate) fn empty() -> Self {
        Self {
            width: 0,
            height: 0,
            coverage: std::sync::Arc::from([]),
            bearing_x: 0.0,
            bearing_y: 0.0,
            outline_mesh: None,
        }
    }
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
