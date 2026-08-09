//! 字体布局与字形光栅化的共享数据类型。

// 复用字体与富文本共享的 UAX #14 断行边界。
use crate::draw::resources::font::line_break::LineBreakMap;

/// 缺字占位（tofu）字形 ID。后端无真实轮廓时由 `FontService::rasterize_glyph` 合成方框。
pub const TOFU_GLYPH_ID: u32 = u32::MAX - 1;

/// 只占 advance、没有可见轮廓的空白字形 ID。
pub(crate) const WHITESPACE_GLYPH_ID: u32 = u32::MAX - 2;

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
        '\u{200b}' => 0.0,
        ' ' => 0.35,
        '\t' => 1.4,
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
    // 单个估算字符保存字符值、宽度和排他源终点。
    type EstimatedScalar = (char, f32, usize);
    // 使用完整文本生成一次标准 UAX #14 边界。
    let breaks = LineBreakMap::new(text);
    // 固化字符序列以正确识别 CRLF 与字符索引。
    let chars = text.chars().collect::<Vec<_>>();
    // 计算当前估算行的完整 advance。
    let line_width = |line: &[EstimatedScalar]| {
        // 聚合全部字符宽度。
        line.iter().map(|(_, width, _)| *width).sum::<f32>()
    };
    // 计算折行结算时去除尾随可折叠空白的可见宽度。
    let visible_width = |line: &[EstimatedScalar]| {
        // 从行尾剔除普通空白但保留 NBSP 等非断空白。
        line.iter()
            // 反向查找最后一个不可折叠字符。
            .rposition(|(ch, _, _)| !LineBreakMap::collapsible_whitespace(*ch))
            // 聚合到最后一个可见字符为止。
            .map_or(0.0, |end| line_width(&line[..=end]))
    };
    // 保存当前尚未结算的逻辑行。
    let mut line = Vec::<EstimatedScalar>::new();
    // 保存全部已结算行中的最大可见宽度。
    let mut widest_line = 0.0f32;
    // 非空输入至少包含一个逻辑行。
    let mut line_count = 1usize;
    // 记录是否发生过宽度驱动的自动折行。
    let mut width_wrapped = false;
    // 仅有限正宽度启用自动折行。
    let wraps = max_width.is_finite() && max_width > 0.0;
    // 从首个 Unicode 标量开始消费。
    let mut char_index = 0usize;
    // 逐个处理逻辑字符并保留 CRLF 原子语义。
    while char_index < chars.len() {
        // 读取当前 Unicode 标量。
        let ch = chars[char_index];
        // 显式换行独立于宽度直接结算当前行。
        if matches!(ch, '\r' | '\n') {
            // 把当前行宽登记到全局最大值。
            widest_line = widest_line.max(line_width(&line));
            // 清空当前行以开始下一逻辑行。
            line.clear();
            // 一个 CRLF 序列只增加一个逻辑行。
            line_count += 1;
            // CR 后紧跟 LF 时整体跳过两个标量。
            char_index += if ch == '\r' && chars.get(char_index + 1) == Some(&'\n') {
                // CRLF 同时消费两个标量。
                2
            // 单独 CR 或 LF 只消费一个标量。
            } else {
                // 推进单个换行标量。
                1
            };
            // 跳过换行字符的宽度处理。
            continue;
        }
        // 使用与既有估算路径相同的字符宽度模型。
        let width = estimated_scalar_width(ch, font_size);
        // 有限宽度下反复寻找当前溢出的最佳 UAX 边界。
        if wraps {
            // 尾部移动到新行后可能仍然溢出，因此允许重复结算。
            loop {
                // 空行或加入当前字符后仍可容纳时停止折行。
                if line.is_empty() || line_width(&line) + width <= max_width {
                    // 当前字符可以进入本行。
                    break;
                }
                // 在当前行内查找最后一个完整 UAX 断行边界。
                let soft_break = line
                    // 遍历当前行全部字符。
                    .iter()
                    // 保留行内字符索引。
                    .enumerate()
                    // 从后向前选择最近边界。
                    .rposition(|(_, (_, _, char_end))| breaks.allows_at(*char_end))
                    // 将字符索引转换为排他切分位置。
                    .map(|index| index + 1);
                // 标准 UAX 机会优先于任何紧急折行。
                if let Some(break_index) = soft_break {
                    // 将断点后的尾部字符移动到下一行。
                    let tail = line.split_off(break_index);
                    // 结算断点前一行的可见宽度。
                    widest_line = widest_line.max(visible_width(&line));
                    // 记录一次自动折行。
                    line_count += 1;
                    // 标记宽度确实触发过折行。
                    width_wrapped = true;
                    // 使用移动后的尾部继续当前行。
                    line = tail;
                    // 重新检查尾部与当前字符是否仍然溢出。
                    continue;
                }
                // 没有标准机会时只允许超长字母数字词紧急断行。
                let emergency_break = line.last().is_some_and(|(_, _, char_end)| {
                    // 前一字符必须紧邻当前字符边界。
                    *char_end == char_index
                        // 共享表必须显式允许该紧急边界。
                        && breaks.emergency_allows_at(char_index)
                });
                // 非法边界宁可保持单行溢出也不能拆开。
                if !emergency_break {
                    // 停止自动折行并让当前字符进入溢出行。
                    break;
                }
                // 结算超长单词的当前前缀。
                widest_line = widest_line.max(line_width(&line));
                // 清空前缀以从当前字符开始下一行。
                line.clear();
                // 记录紧急自动折行。
                line_count += 1;
                // 标记宽度确实触发过折行。
                width_wrapped = true;
            }
        }
        // 将当前字符加入尚未结算的逻辑行。
        line.push((ch, width, char_index + 1));
        // 推进到下一个 Unicode 标量。
        char_index += 1;
    }
    // 返回估算布局的最大宽度、行数和自动折行标记。
    EstimatedTextMetrics {
        // 最后一行使用完整宽度，保持无折行度量的尾随空白契约。
        max_line_width: widest_line.max(line_width(&line)),
        // 返回强制与自动换行共同形成的行数。
        line_count,
        // 返回是否发生过宽度折行。
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
    /// 当前 shaping cluster 在源文本中的排他字符终点。
    pub char_end: usize,
    pub font: crate::draw::FontHandle,
}

/// 返回字符区间在一行字形中的可见水平范围。
///
/// 选择区使用源字符下标，而不是假设一个 Unicode 标量必然对应一个字形槽；
/// 同时以最小/最大边界兼容未来后端返回视觉顺序字形。
pub(crate) fn glyph_selection_x_range(
    glyphs: &[PositionedGlyph],
    start_char: usize,
    end_char: usize,
) -> Option<(f32, f32)> {
    let mut left = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;
    for glyph in glyphs
        .iter()
        // 选择区与 cluster 源区间相交时纳入完整字形几何。
        .filter(|glyph| glyph.char_index < end_char && glyph.char_end > start_char)
    {
        left = left.min(glyph.x);
        right = right.max(glyph.x + glyph.width.max(0.0));
    }
    (left.is_finite() && right.is_finite()).then_some((left, right.max(left)))
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
