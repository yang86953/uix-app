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

// 无自动折行时单次流式统计显式行，避免构造断行表、字符表和当前行缓冲。
fn estimate_unwrapped_text_metrics(text: &str, font_size: f32) -> EstimatedTextMetrics {
    let mut widest_line = 0.0_f32;
    let mut current_width = 0.0_f32;
    let mut line_count = 1_usize;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if matches!(ch, '\r' | '\n') {
            widest_line = widest_line.max(current_width);
            current_width = 0.0;
            line_count += 1;
            // CRLF 是一个显式换行，不得重复增加逻辑行数。
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
        } else {
            current_width += estimated_scalar_width(ch, font_size);
        }
    }
    EstimatedTextMetrics {
        max_line_width: widest_line.max(current_width),
        line_count,
        width_wrapped: false,
    }
}

/// 无字体上下文时使用的文本尺寸估算；显式换行与 CJK 行首禁则须与真实布局一致。
pub(crate) fn estimate_text_metrics(
    text: &str,
    max_width: f32,
    font_size: f32,
) -> EstimatedTextMetrics {
    // 非有限或非正宽度不启用自动折行，直接走无分配流式路径。
    let wraps = max_width.is_finite() && max_width > 0.0;
    if !wraps {
        return estimate_unwrapped_text_metrics(text, font_size);
    }
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

/// 描述一次 OpenType shaping 使用的已解析行内方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDirection {
    /// 指定从左向右进行字形塑形。
    LeftToRight,
    /// 指定从右向左进行字形塑形。
    RightToLeft,
}

/// A single glyph positioned by text layout.
#[derive(Debug, Clone, Copy)]
pub struct PositionedGlyph {
    /// 字形边界框左缘在布局坐标系中的横坐标。
    pub x: f32,
    /// 字形边界框上缘在布局坐标系中的纵坐标。
    pub y: f32,
    /// 字形边界框的布局宽度。
    pub width: f32,
    /// 字形边界框的布局高度。
    pub height: f32,
    /// 字体内部用于光栅化的字形标识。
    pub glyph_id: u32,
    /// 对应源文本中的 Unicode 标量下标（`chars()` 序），与 glyph 下标解耦。
    pub char_index: usize,
    /// 当前 shaping cluster 在源文本中的排他字符终点。
    pub char_end: usize,
    /// 当前视觉行应用 UAX #9 L1 后的嵌入级别；奇数表示 RTL。
    pub bidi_level: u8,
    /// 生成该字形时选中的字体资源句柄。
    pub font: crate::draw::FontHandle,
}

/// 返回逻辑选择区在视觉行中的全部连续水平片段。
pub(crate) fn visit_glyph_selection_x_ranges(
    // 借用按视觉顺序排列的行字形。
    glyphs: &[PositionedGlyph],
    // 指定逻辑选择起点。
    start_char: usize,
    // 指定逻辑选择排他终点。
    end_char: usize,
    // 每发现一个连续视觉片段就立即交给调用方。
    mut visit: impl FnMut(f32, f32),
) {
    // 栈上保存当前连续片段，避免稳态绘制创建临时向量。
    let mut current: Option<(f32, f32)> = None;
    // 依次观察视觉字形，逻辑区间相交时纳入选择。
    for glyph in glyphs.iter().filter(|glyph| {
        // cluster 源区间与逻辑选择区间相交。
        glyph.char_index < end_char && glyph.char_end > start_char
    }) {
        // 当前字形可见左缘。
        let left = glyph.x;
        // 当前字形可见右缘。
        let right = (glyph.x + glyph.width.max(0.0)).max(left);
        // 与当前视觉片段接触或重叠时直接扩展右缘。
        if let Some((_, current_right)) = current.as_mut().filter(|range| left <= range.1 + 0.01) {
            // 保留所有重叠字形的最远右缘。
            *current_right = current_right.max(right);
        } else {
            // 新片段开始前先提交已经闭合的片段。
            if let Some((current_left, current_right)) = current.take() {
                visit(current_left, current_right);
            }
            // 在栈上开始新的连续视觉片段。
            current = Some((left, right));
        }
    }
    // 提交行尾仍未闭合的最后一个片段。
    if let Some((left, right)) = current {
        visit(left, right);
    }
}

/// 返回逻辑选择区在视觉行中的全部连续水平片段。
pub(crate) fn glyph_selection_x_ranges(
    // 借用按视觉顺序排列的行字形。
    glyphs: &[PositionedGlyph],
    // 指定逻辑选择起点。
    start_char: usize,
    // 指定逻辑选择排他终点。
    end_char: usize,
) -> Vec<(f32, f32)> {
    // 保存可能因双向 run 分离而形成的多个视觉片段。
    let mut ranges: Vec<(f32, f32)> = Vec::new();
    // 兼容需要持有结果的调用方，同时让绘制热路径复用流式实现。
    visit_glyph_selection_x_ranges(glyphs, start_char, end_char, |left, right| {
        // 固化当前连续视觉片段。
        ranges.push((left, right));
    });
    // 返回同一视觉布局派生的全部选择片段。
    ranges
}

/// 在一行视觉字形中命中逻辑光标边界。
pub(crate) fn glyph_hit_test_index(glyphs: &[PositionedGlyph], x: f32) -> Option<usize> {
    // 空视觉行没有可命中的 cluster。
    if glyphs.is_empty() {
        // 返回空值交由行信息兜底。
        return None;
    }
    // 从首个视觉字形开始按 cluster 分组。
    let mut start = 0usize;
    // 记录最后一个视觉 cluster 的逻辑右侧边界。
    let mut trailing_boundary = None;
    // 遍历当前视觉行全部 cluster。
    while start < glyphs.len() {
        // 当前 cluster 的完整逻辑源区间。
        let source_range = glyphs[start].char_index..glyphs[start].char_end;
        // 查找相邻同源区间字形的排他终点。
        let mut end = start + 1;
        // 同一 shaping cluster 的多个字形必须共享命中区域。
        while end < glyphs.len()
            && glyphs[end].char_index == source_range.start
            && glyphs[end].char_end == source_range.end
        {
            // 扩展 cluster 字形范围。
            end += 1;
        }
        // 聚合 cluster 视觉左缘。
        let left = glyphs[start..end]
            // 遍历 cluster 字形。
            .iter()
            // 提取水平坐标。
            .map(|glyph| glyph.x)
            // 聚合最小值。
            .fold(f32::INFINITY, f32::min);
        // 聚合 cluster 视觉右缘。
        let right = glyphs[start..end]
            // 遍历 cluster 字形。
            .iter()
            // 提取非负右缘。
            .map(|glyph| glyph.x + glyph.width.max(0.0))
            // 聚合最大值。
            .fold(f32::NEG_INFINITY, f32::max);
        // 奇数嵌入级别表示视觉左侧对应逻辑排他终点。
        let rtl = glyphs[start].bidi_level % 2 == 1;
        // 指针位于 cluster 中点左侧时返回其视觉左边界。
        if x < left + (right - left).max(0.0) * 0.5 {
            // RTL 与 LTR 的视觉左边界对应相反逻辑边界。
            return Some(if rtl {
                // RTL 左缘对应逻辑排他终点。
                source_range.end
            // LTR 左缘对应逻辑起点。
            } else {
                // LTR 左缘对应逻辑起点。
                source_range.start
            });
        }
        // 保存当前 cluster 视觉右缘对应的逻辑边界。
        trailing_boundary = Some(if rtl {
            // RTL 右缘对应逻辑起点。
            source_range.start
        // LTR 右缘对应逻辑排他终点。
        } else {
            // LTR 右缘对应逻辑排他终点。
            source_range.end
        });
        // 继续下一个视觉 cluster。
        start = end;
    }
    // 行右侧命中返回最后一个视觉 cluster 的右边界。
    trailing_boundary
}

/// 返回逻辑字符边界在一行视觉字形中的主光标 x 坐标。
pub(crate) fn glyph_cursor_x(
    // 借用按视觉顺序排列的行字形。
    glyphs: &[PositionedGlyph],
    // 指定逻辑光标边界。
    char_index: usize,
) -> Option<f32> {
    // 从视觉行首开始按 shaping cluster 分组。
    let mut start = 0usize;
    // 遍历全部 cluster 查找共享逻辑边界。
    while start < glyphs.len() {
        // 保存当前 cluster 源区间。
        let source_range = glyphs[start].char_index..glyphs[start].char_end;
        // 查找相邻同源区间字形终点。
        let mut end = start + 1;
        // 完整 cluster 共享一个光标边界对。
        while end < glyphs.len()
            && glyphs[end].char_index == source_range.start
            && glyphs[end].char_end == source_range.end
        {
            // 扩展 cluster 字形范围。
            end += 1;
        }
        // 当前逻辑边界不接触此 cluster 时继续。
        if char_index != source_range.start && char_index != source_range.end {
            // 跳到下一个 cluster。
            start = end;
            // 继续扫描。
            continue;
        }
        // 聚合 cluster 视觉左缘。
        let left = glyphs[start..end]
            // 遍历 cluster 字形。
            .iter()
            // 提取水平坐标。
            .map(|glyph| glyph.x)
            // 聚合最小值。
            .fold(f32::INFINITY, f32::min);
        // 聚合 cluster 视觉右缘。
        let right = glyphs[start..end]
            // 遍历 cluster 字形。
            .iter()
            // 提取可见右缘。
            .map(|glyph| glyph.x + glyph.width.max(0.0))
            // 聚合最大值。
            .fold(f32::NEG_INFINITY, f32::max);
        // 奇数嵌入级别交换逻辑起止边界的视觉侧。
        let rtl = glyphs[start].bidi_level % 2 == 1;
        // 返回当前逻辑边界对应的视觉坐标。
        return Some(if char_index == source_range.start {
            // 逻辑起点在 RTL cluster 右侧、LTR cluster 左侧。
            if rtl {
                // RTL 起点使用视觉右缘。
                right
            // LTR 起点使用视觉左缘。
            } else {
                // LTR 起点使用视觉左缘。
                left
            }
        // 当前边界等于 cluster 排他终点。
        } else if rtl {
            // RTL 终点使用视觉左缘。
            left
        // LTR 终点使用视觉右缘。
        } else {
            // LTR 终点使用视觉右缘。
            right
        });
    }
    // 当前行没有覆盖指定逻辑边界。
    None
}

/// 一行文本的布局信息。
#[derive(Debug, Clone, Copy)]
pub struct LineInfo {
    /// 行顶缘在文本布局坐标系中的纵坐标。
    pub y: f32,
    /// 行框高度。
    pub height: f32,
    /// 行内已排版内容的水平宽度。
    pub width: f32,
    /// 该行覆盖的首个 Unicode 标量下标。
    pub start_char: usize,
    /// 该行覆盖范围的排他 Unicode 标量终点。
    pub end_char: usize,
    /// 该行首个字形在布局字形数组中的下标。
    pub glyph_start: usize,
    /// 该行包含的连续字形数量。
    pub glyph_count: usize,
}

/// 文本布局结果。
#[derive(Debug, Clone)]
pub struct TextLayout {
    /// 按视觉绘制顺序定位后的全部字形。
    pub glyphs: Vec<PositionedGlyph>,
    /// 按垂直布局顺序记录的文本行。
    pub lines: Vec<LineInfo>,
    /// 最终文本布局边界的宽度。
    pub width: f32,
    /// 最终文本布局边界的高度。
    pub height: f32,
}

/// 光栅化字形数据。
///
/// `outline_mesh` 为 NonZero 边列表 `[ax,ay,bx,by,…]`（本地像素，含 MSDF 范围 fringe）；
/// 同时缓存字体光栅器的面积 `coverage` 供物理 1:1 R8 路径复用。严格 GPU
/// 缩放、仿射或高 DPR 走 RGBA8 MSDF。
#[derive(Debug, Clone)]
pub struct GlyphRaster {
    /// 光栅覆盖图的像素宽度。
    pub width: usize,
    /// 光栅覆盖图的像素高度。
    pub height: usize,
    /// 按行优先排列的单通道像素覆盖率。
    pub coverage: std::sync::Arc<[u8]>,
    /// 字形光栅左缘相对排版原点的水平偏移。
    pub bearing_x: f32,
    /// 字形光栅上缘相对排版基线的垂直偏移。
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
    /// 基线到字体上缘的正向距离。
    pub ascent: f32,
    /// 基线到字体下缘的正向距离。
    pub descent: f32,
    /// 相邻文本基线之间的默认垂直距离。
    pub new_line_size: f32,
}

/// Text layout options.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLayoutOptions {
    /// 文本布局允许占用的最大宽度。
    pub max_width: f32,
    /// 文本布局允许占用的最大高度。
    pub max_height: f32,
    /// 相邻文本行使用的行框高度。
    pub line_height: f32,
    /// 是否按可用宽度自动折行。
    pub word_wrap: bool,
    /// 各行内容在水平可用空间中的对齐方式。
    pub h_align: crate::draw::HAlign,
    /// 整体文本在垂直可用空间中的对齐方式。
    pub v_align: crate::draw::VAlign,
    /// 塑形与光栅化使用的字体像素大小。
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
