//! 富文本布局引擎模块。
//!
//! 提供字符级文本布局、断行、字形缓存等功能。
//! 从 `rich_text.rs` 拆分出来以遵守 900 行文件限制。

// 复用富文本的段类型定义。
use super::RichTextSegment;
// 复用独立的布局字形、行与代码复制区域类型。
use super::layout_types::{LayoutGlyph, LayoutLine};
// 复用独立的估算字符宽度、行刷新与完整逻辑源拼接。
use super::layout_metrics::{char_width, flush_line, source_text};
// 复用独立的真实字体逐字符度量逻辑。
use super::shaped_advance::real_char_advances;
// 测试直接验证 shaping cluster advance 映射。
#[cfg(test)]
use super::shaped_advance::measured_advance_for_char;
use crate::draw::resources::font::font_service::FontService;
// 让估算与真实富文本布局共享 UAX #14 断行边界和强制换行切分。
use crate::draw::resources::font::line_break::{split_once_mandatory, LineBreakMap};
// 测试使用后端字形构造稀疏索引和 kerning 场景。
#[cfg(test)]
use crate::draw::resources::font::text_backend::PositionedGlyph;
use crate::draw::{Color, FontHandle, Transform};
use crate::ui::component::paint_context::PaintContext;

// 统一定义富文本 faux italic 的倾斜比例。
const RICH_TEXT_ITALIC_SHEAR: f32 = 0.18;

// 绘制一个连续富文本 run，并在需要时应用粗体和斜体样式。
pub(crate) fn draw_rich_text_run(
    ctx: &mut PaintContext,
    content: &str,
    pos: crate::core::Point,
    color: Color,
    font_size: f32,
    segment: Option<&RichTextSegment>,
) {
    // 从段模型读取当前 run 的文本样式。
    let style = match segment {
        // Text 段携带粗体和斜体等样式覆盖。
        Some(RichTextSegment::Text { style, .. }) => Some(style),
        // Link、Code 和换行段不使用 Text 样式覆盖。
        _ => None,
    };
    // 读取当前 run 是否需要斜体倾斜。
    let italic = style.is_some_and(|style| style.italic);
    // 读取当前 run 是否需要粗体加描边。
    let bold = style.is_some_and(|style| style.bold);
    // 斜体只影响当前 run，先保存继承的画布变换状态。
    if italic {
        // 保存当前绘制状态，避免倾斜泄漏到后续 run。
        ctx.save();
        // 以文本顶部为轴应用局部水平剪切。
        ctx.concat_transform(italic_transform(pos.y));
    }
    // 使用既有字体服务绘制原始 run，保持测量和光栅化入口一致。
    ctx.draw_text(content, pos, color, font_size);
    // 沿用现有粗体策略，避免改变已验证的字宽与字体选择。
    if bold {
        // 通过轻微水平偏移叠加字形形成粗体视觉效果。
        ctx.draw_text(
            content,
            crate::core::Point::new(pos.x + 0.6, pos.y),
            color,
            font_size,
        );
    }
    // 斜体 run 绘制结束后恢复外层画布状态。
    if italic {
        // 恢复保存前的变换，确保链接、代码和普通文本不受影响。
        ctx.restore();
    }
}

// 返回围绕文本顶部的局部斜体仿射变换。
pub(crate) fn italic_transform(pivot_y: f32) -> Transform {
    // 非有限坐标回退到原点，避免把无效状态写入绘制命令。
    let pivot_y = if pivot_y.is_finite() { pivot_y } else { 0.0 };
    // 先移到局部轴，再剪切，最后移回原坐标系。
    Transform::translate(0.0, pivot_y)
        .concat(Transform {
            m: [1.0, RICH_TEXT_ITALIC_SHEAR, 0.0, 0.0, 1.0, 0.0],
        })
        .concat(Transform::translate(0.0, -pivot_y))
}

// 验证斜体变换只倾斜字形，不改变文本顶部的定位。
#[cfg(test)]
mod tests {
    // 引入当前布局模块中的测试辅助函数和几何类型。
    use super::*;

    // 验证顶部锚点保持不动且下缘向右倾斜。
    #[test]
    fn italic_transform_keeps_top_edge_and_slants_lower_edge() {
        // 构造一个以 y=10 为顶部轴的斜体变换。
        let transform = italic_transform(10.0);
        // 计算顶部点经过变换后的坐标。
        let top = transform.transform_point(crate::core::Point::new(5.0, 10.0));
        // 计算下方点经过变换后的坐标。
        let lower = transform.transform_point(crate::core::Point::new(5.0, 20.0));
        // 顶部点的水平坐标应保持不变。
        assert!((top.x - 5.0).abs() < f32::EPSILON);
        // 顶部点的垂直坐标应保持不变。
        assert!((top.y - 10.0).abs() < f32::EPSILON);
        // 下方点应按 0.18 的倾斜比例向右移动。
        assert!((lower.x - 6.8).abs() < 0.0001);
        // 下方点的垂直坐标应保持不变。
        assert!((lower.y - 20.0).abs() < f32::EPSILON);
    }
    // 验证 Unicode 空白作为独立 token 时，窄宽度换行不会拆散后续单词。
    #[test]
    fn unicode_whitespace_wraps_as_a_separate_token() {
        // 构造前后两个普通文本段，模拟内联元素边界后的空白正文。
        let segments = vec![
            RichTextSegment::Text {
                content: "a".into(),
                style: Default::default(),
            },
            RichTextSegment::Text {
                content: "\u{2003}bb".into(),
                style: Default::default(),
            },
        ];
        // 使用窄宽度使空白可以留在第一行而后续单词换到第二行。
        let (lines, _, _) = layout_rich_text(&segments, 12.0, 10.0, Color::black());
        // 断行结果应保持为两行而不是把空白与单词一起推入逐字回退。
        assert_eq!(lines.len(), 2);
        // 第一行应保留源文本中的字母和 Unicode 空白。
        let first: String = lines[0].glyphs.iter().map(|glyph| glyph.ch).collect();
        // 第二行应只包含后续单词。
        let second: String = lines[1].glyphs.iter().map(|glyph| glyph.ch).collect();
        // 两行字符顺序应与源文本一致。
        assert_eq!(first, "a\u{2003}");
        // 后续单词不应被空白 token 牵连到上一行。
        assert_eq!(second, "bb");
    }

    // 验证真实 advance 使用源字符索引，而不是依赖 glyph 数组槽位。
    #[test]
    fn real_advance_uses_source_char_index() {
        // 构造跳过一个源字符索引的后端字形序列。
        let glyphs = vec![
            PositionedGlyph {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 12.0,
                glyph_id: 1,
                char_index: 0,
                // 测试字形覆盖第一个源字符。
                char_end: 1,
                font: FontHandle::new(0),
            },
            PositionedGlyph {
                x: 10.0,
                y: 0.0,
                width: 20.0,
                height: 12.0,
                glyph_id: 2,
                char_index: 2,
                // 测试字形覆盖第三个源字符。
                char_end: 3,
                font: FontHandle::new(0),
            },
            PositionedGlyph {
                x: 30.0,
                y: 0.0,
                width: 0.0,
                height: 12.0,
                glyph_id: 3,
                char_index: 3,
                // 测试字形覆盖第四个源字符。
                char_end: 4,
                font: FontHandle::new(0),
            },
        ];
        // 索引 2 应读取第二个字形的真实宽度，而不是索引 1 的槽位。
        assert_eq!(measured_advance_for_char(&glyphs, 2, 7.0), 20.0);
        // 后端明确返回的零宽字形不能被误回退为可见宽度。
        assert_eq!(measured_advance_for_char(&glyphs, 3, 7.0), 0.0);
        // 没有字形的索引应回退到估算宽度。
        assert_eq!(measured_advance_for_char(&glyphs, 1, 7.0), 7.0);
        // 构造相邻字形的负 kerning，验证实际 x 间距会缩短当前 advance。
        let kerned_glyphs = vec![
            // 第一个字形的 advance 应由下一个源字形的 x 坐标决定。
            PositionedGlyph {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 12.0,
                glyph_id: 4,
                char_index: 0,
                // 测试字形覆盖第一个源字符。
                char_end: 1,
                font: FontHandle::new(0),
            },
            // 下一个字形左移到 9px，模拟字体后端返回的 kerning。
            PositionedGlyph {
                x: 9.0,
                y: 0.0,
                width: 8.0,
                height: 12.0,
                glyph_id: 5,
                char_index: 1,
                // 测试字形覆盖第二个源字符。
                char_end: 2,
                font: FontHandle::new(0),
            },
        ];
        // 富文本游标应与最终 draw_text 的下一个字形起点保持一致。
        assert_eq!(measured_advance_for_char(&kerned_glyphs, 0, 7.0), 9.0);
    }
}

// ══════════════════════════════════════════════════════════════════
// 估算布局（不依赖 FontService）
// ══════════════════════════════════════════════════════════════════

/// 执行富文本布局（字符宽度估算版本）
///
/// 返回 (行列表, 总高度, 最大行宽)。
pub(crate) fn layout_rich_text(
    segments: &[RichTextSegment],
    max_width: f32,
    default_font_size: f32,
    default_color: Color,
) -> (Vec<LayoutLine>, f32, f32) {
    // 以完整富文本源生成跨样式段共享的 UAX #14 边界。
    let full_source = source_text(segments);
    // 同一边界表贯穿 Text、Code 与 Link 段。
    let breaks = LineBreakMap::new(&full_source);
    let mut lines: Vec<LayoutLine> = Vec::new();
    let mut current_line_glyphs: Vec<LayoutGlyph> = Vec::new();
    let mut current_x: f32 = 0.0;
    let line_height_factor: f32 = 1.5;
    let default_line_h = default_font_size * line_height_factor;
    let mut current_line_h: f32 = default_line_h;
    let mut max_line_w: f32 = 0.0;
    // 保存当前 segment 在完整逻辑源中的字符起点。
    let mut source_offset = 0usize;

    for (seg_idx, segment) in segments.iter().enumerate() {
        match segment {
            RichTextSegment::NewLine => {
                max_line_w = max_line_w.max(current_x);
                flush_line(&mut lines, &mut current_line_glyphs, current_line_h);
                current_x = 0.0;
                current_line_h = default_line_h;
                // 显式换行在完整逻辑源中占一个字符位置。
                source_offset += 1;
            }
            RichTextSegment::Text { content, style } => {
                let fs = style.resolved_font_size(default_font_size);
                let color = style.resolved_color(default_color);
                let bg = style.bg_color;
                let seg_line_h = fs * line_height_factor;
                current_line_h = current_line_h.max(seg_line_h);

                layout_text_content(
                    content,
                    fs,
                    color,
                    bg,
                    false,
                    None,
                    seg_idx,
                    max_width,
                    seg_line_h,
                    &mut current_line_h,
                    default_line_h,
                    &mut lines,
                    &mut current_line_glyphs,
                    &mut current_x,
                    &mut max_line_w,
                    // 传入完整源断行表。
                    &breaks,
                    // 传入当前文本段全局字符起点。
                    source_offset,
                );
                // 推进到下一 segment 的全局字符起点。
                source_offset += content.chars().count();
            }
            RichTextSegment::Code { content } => {
                let fs = default_font_size * 0.9;
                let color = Color::from_rgb(230, 180, 100);
                let bg = Color::from_rgb(40, 40, 45);
                let seg_line_h = fs * line_height_factor;
                current_line_h = current_line_h.max(seg_line_h);

                layout_text_content(
                    content,
                    fs,
                    color,
                    Some(bg),
                    false,
                    None,
                    seg_idx,
                    max_width,
                    seg_line_h,
                    &mut current_line_h,
                    default_line_h,
                    &mut lines,
                    &mut current_line_glyphs,
                    &mut current_x,
                    &mut max_line_w,
                    // 传入完整源断行表。
                    &breaks,
                    // 传入当前代码段全局字符起点。
                    source_offset,
                );
                // 推进到下一 segment 的全局字符起点。
                source_offset += content.chars().count();
            }
            RichTextSegment::Link { content, url } => {
                let fs = default_font_size;
                let color = Color::from_rgb(55, 110, 255);
                let seg_line_h = fs * line_height_factor;
                current_line_h = current_line_h.max(seg_line_h);

                layout_text_content(
                    content,
                    fs,
                    color,
                    None,
                    true,
                    Some(url.as_str()),
                    seg_idx,
                    max_width,
                    seg_line_h,
                    &mut current_line_h,
                    default_line_h,
                    &mut lines,
                    &mut current_line_glyphs,
                    &mut current_x,
                    &mut max_line_w,
                    // 传入完整源断行表。
                    &breaks,
                    // 传入当前链接段全局字符起点。
                    source_offset,
                );
                // 推进到下一 segment 的全局字符起点。
                source_offset += content.chars().count();
            }
        }
    }

    max_line_w = max_line_w.max(current_x);
    if !current_line_glyphs.is_empty() || lines.is_empty() {
        flush_line(&mut lines, &mut current_line_glyphs, current_line_h);
    }

    let total_height = lines
        .last()
        .map(|l| l.y + l.height)
        .unwrap_or(default_line_h);
    (lines, total_height, max_line_w)
}

/// 布局一段文本内容（估算宽度版本）
#[allow(clippy::too_many_arguments)]
fn layout_text_content(
    content: &str,
    fs: f32,
    color: Color,
    bg_color: Option<Color>,
    is_link: bool,
    link_url: Option<&str>,
    seg_idx: usize,
    max_width: f32,
    seg_line_h: f32,
    current_line_h: &mut f32,
    default_line_h: f32,
    lines: &mut Vec<LayoutLine>,
    glyphs: &mut Vec<LayoutGlyph>,
    current_x: &mut f32,
    max_line_w: &mut f32,
    breaks: &LineBreakMap,
    source_offset: usize,
) {
    // 按显式换行把内容拆成多个逻辑行，保证估算布局与真实布局共享换行语义。
    let mut remaining = content;
    // 保存当前逻辑行相对于 segment 起点已消费的字符数量。
    let mut consumed_chars = 0usize;
    // 循环消费当前段中的每一行，保留末尾空行的边界行为。
    loop {
        // 只在当前行存在换行时切出后续内容。
        // 使用共享辅助整体消费 CRLF、CR 或 LF。
        let (line, next) = split_once_mandatory(remaining);
        // 计算当前逻辑行在完整富文本源中的字符起点。
        let line_source_offset = source_offset + consumed_chars;
        // 使用原有空白 token 逻辑布局当前行。
        layout_text_content_line(
            line,
            fs,
            color,
            bg_color,
            is_link,
            link_url,
            seg_idx,
            max_width,
            seg_line_h,
            lines,
            glyphs,
            current_x,
            max_line_w,
            // 传入跨 segment 共享的断行表。
            breaks,
            // 传入当前逻辑行全局字符起点。
            line_source_offset,
        );
        // 没有后续换行时当前段布局完成。
        let Some(next) = next else {
            // 退出循环并保留最后一行的当前游标。
            break;
        };
        // 显式换行前先结算当前行的最大宽度。
        *max_line_w = (*max_line_w).max(*current_x);
        // 使用当前行实际高度刷新行列表。
        flush_line(lines, glyphs, (*current_line_h).max(seg_line_h));
        // 换行后从行首重新开始布局。
        *current_x = 0.0;
        // 新行至少保留默认行高和当前段行高中的较大值。
        *current_line_h = default_line_h.max(seg_line_h);
        // 计算本轮逻辑行和强制换行分隔符共同消费的字符数量。
        consumed_chars += remaining.chars().count() - next.chars().count();
        // 继续处理换行后的剩余内容。
        remaining = next;
    }
}

/// 使用 UAX #14 机会和受控长单词兜底布局单个逻辑行。
#[allow(clippy::too_many_arguments)]
fn layout_text_content_line(
    content: &str,
    fs: f32,
    color: Color,
    bg_color: Option<Color>,
    is_link: bool,
    link_url: Option<&str>,
    seg_idx: usize,
    max_width: f32,
    seg_line_h: f32,
    lines: &mut Vec<LayoutLine>,
    glyphs: &mut Vec<LayoutGlyph>,
    current_x: &mut f32,
    max_line_w: &mut f32,
    breaks: &LineBreakMap,
    source_offset: usize,
) {
    let shared_url: Option<std::sync::Arc<str>> = link_url.map(std::sync::Arc::from);
    // 固化逻辑字符以按字符索引查询 UAX 边界。
    let chars = content.chars().collect::<Vec<_>>();
    // 从首个字符开始消费相邻 UAX 机会之间的原子片段。
    let mut start = 0usize;
    // 逐段消费直到完整逻辑行结束。
    while start < chars.len() {
        // 至少把当前字符纳入片段。
        let mut end = start + 1;
        // 扩展到下一个标准允许或强制边界。
        while end < chars.len() && !breaks.allows_at(source_offset + end) {
            // 继续保留禁止断行的后继字符。
            end += 1;
        }
        // 计算当前 UAX 原子片段的估算宽度。
        let token_w = chars[start..end]
            // 遍历片段字符。
            .iter()
            // 使用共享字符宽度估算。
            .map(|ch| char_width(fs, *ch))
            // 聚合完整片段宽度。
            .sum::<f32>();

        if breaks.allows_at(source_offset + start)
            && *current_x + token_w > max_width
            && !glyphs.is_empty()
        {
            *max_line_w = (*max_line_w).max(*current_x);
            flush_line(lines, glyphs, seg_line_h);
            *current_x = 0.0;
        }

        if *current_x + token_w > max_width {
            let mut word_chars_x = *current_x;
            for (relative_index, ch) in chars[start..end].iter().enumerate() {
                let cw = char_width(fs, *ch);
                // 只有长字母数字词的紧急边界可以绕过标准 UAX 机会。
                let emergency_break =
                    breaks.emergency_allows_at(source_offset + start + relative_index);
                if word_chars_x + cw > max_width
                    && word_chars_x > 0.0
                    && !glyphs.is_empty()
                    && emergency_break
                {
                    *max_line_w = (*max_line_w).max(word_chars_x);
                    flush_line(lines, glyphs, seg_line_h);
                    word_chars_x = 0.0;
                }
                glyphs.push(LayoutGlyph {
                    segment_idx: seg_idx,
                    // 直接保存完整源字符索引，保留 CRLF 与跨样式段偏移。
                    global_char_idx: source_offset + start + relative_index,
                    ch: *ch,
                    x: word_chars_x,
                    width: cw,
                    font_size: fs,
                    color,
                    bg_color,
                    is_link,
                    link_url: shared_url.clone(),
                });
                word_chars_x += cw;
            }
            *current_x = word_chars_x;
        } else {
            for (relative_index, ch) in chars[start..end].iter().enumerate() {
                let cw = char_width(fs, *ch);
                glyphs.push(LayoutGlyph {
                    segment_idx: seg_idx,
                    // 直接保存完整源字符索引，保留 CRLF 与跨样式段偏移。
                    global_char_idx: source_offset + start + relative_index,
                    ch: *ch,
                    x: *current_x,
                    width: cw,
                    font_size: fs,
                    color,
                    bg_color,
                    is_link,
                    link_url: shared_url.clone(),
                });
                *current_x += cw;
            }
        }
        // 从当前 UAX 边界继续处理后续片段。
        start = end;
    }
}

// ══════════════════════════════════════════════════════════════════
// 真实字体度量布局（用于 render 阶段）
// ══════════════════════════════════════════════════════════════════

/// 基于真实字体度量执行富文本布局
pub(crate) fn layout_rich_text_real(
    segments: &[RichTextSegment],
    max_width: f32,
    default_font_size: f32,
    default_color: Color,
    font_service: &FontService,
    font: &FontHandle,
) -> (Vec<LayoutLine>, f32, f32) {
    // 以完整富文本源生成跨样式段共享的 UAX #14 边界。
    let full_source = source_text(segments);
    // 同一边界表贯穿 Text、Code 与 Link 的真实字体路径。
    let breaks = LineBreakMap::new(&full_source);
    let mut lines: Vec<LayoutLine> = Vec::new();
    let mut current_line_glyphs: Vec<LayoutGlyph> = Vec::new();
    let mut current_x: f32 = 0.0;
    let line_height_factor: f32 = 1.5;
    let default_line_h = default_font_size * line_height_factor;
    let mut current_line_h: f32 = default_line_h;
    let mut max_line_w: f32 = 0.0;
    // 保存当前 segment 在完整逻辑源中的字符起点。
    let mut source_offset = 0usize;

    for (seg_idx, segment) in segments.iter().enumerate() {
        match segment {
            RichTextSegment::NewLine => {
                max_line_w = max_line_w.max(current_x);
                flush_line(&mut lines, &mut current_line_glyphs, current_line_h);
                current_x = 0.0;
                current_line_h = default_line_h;
                // 显式换行在完整逻辑源中占一个字符位置。
                source_offset += 1;
            }
            RichTextSegment::Text { content, style } => {
                let fs = style.resolved_font_size(default_font_size);
                let color = style.resolved_color(default_color);
                let bg = style.bg_color;
                let seg_line_h = fs * line_height_factor;
                current_line_h = current_line_h.max(seg_line_h);

                layout_text_content_real(
                    content,
                    fs,
                    color,
                    bg,
                    false,
                    None,
                    seg_idx,
                    max_width,
                    seg_line_h,
                    &mut current_line_h,
                    default_line_h,
                    &mut lines,
                    &mut current_line_glyphs,
                    &mut current_x,
                    &mut max_line_w,
                    font_service,
                    font,
                    // 传入完整源断行表。
                    &breaks,
                    // 传入当前文本段全局字符起点。
                    source_offset,
                );
                // 推进到下一 segment 的全局字符起点。
                source_offset += content.chars().count();
            }
            RichTextSegment::Code { content } => {
                let fs = default_font_size * 0.9;
                let color = Color::from_rgb(230, 180, 100);
                let bg = Color::from_rgb(40, 40, 45);
                let seg_line_h = fs * line_height_factor;
                current_line_h = current_line_h.max(seg_line_h);

                layout_text_content_real(
                    content,
                    fs,
                    color,
                    Some(bg),
                    false,
                    None,
                    seg_idx,
                    max_width,
                    seg_line_h,
                    &mut current_line_h,
                    default_line_h,
                    &mut lines,
                    &mut current_line_glyphs,
                    &mut current_x,
                    &mut max_line_w,
                    font_service,
                    font,
                    // 传入完整源断行表。
                    &breaks,
                    // 传入当前代码段全局字符起点。
                    source_offset,
                );
                // 推进到下一 segment 的全局字符起点。
                source_offset += content.chars().count();
            }
            RichTextSegment::Link { content, url } => {
                let fs = default_font_size;
                let color = Color::from_rgb(55, 110, 255);
                let seg_line_h = fs * line_height_factor;
                current_line_h = current_line_h.max(seg_line_h);

                layout_text_content_real(
                    content,
                    fs,
                    color,
                    None,
                    true,
                    Some(url.as_str()),
                    seg_idx,
                    max_width,
                    seg_line_h,
                    &mut current_line_h,
                    default_line_h,
                    &mut lines,
                    &mut current_line_glyphs,
                    &mut current_x,
                    &mut max_line_w,
                    font_service,
                    font,
                    // 传入完整源断行表。
                    &breaks,
                    // 传入当前链接段全局字符起点。
                    source_offset,
                );
                // 推进到下一 segment 的全局字符起点。
                source_offset += content.chars().count();
            }
        }
    }

    max_line_w = max_line_w.max(current_x);
    if !current_line_glyphs.is_empty() || lines.is_empty() {
        flush_line(&mut lines, &mut current_line_glyphs, current_line_h);
    }

    let total_height = lines
        .last()
        .map(|l| l.y + l.height)
        .unwrap_or(default_line_h);
    (lines, total_height, max_line_w)
}

/// 基于真实字体度量布局一段文本
#[allow(clippy::too_many_arguments)]
fn layout_text_content_real(
    content: &str,
    fs: f32,
    color: Color,
    bg_color: Option<Color>,
    is_link: bool,
    link_url: Option<&str>,
    seg_idx: usize,
    max_width: f32,
    seg_line_h: f32,
    current_line_h: &mut f32,
    default_line_h: f32,
    lines: &mut Vec<LayoutLine>,
    glyphs: &mut Vec<LayoutGlyph>,
    current_x: &mut f32,
    max_line_w: &mut f32,
    font_service: &FontService,
    font: &FontHandle,
    breaks: &LineBreakMap,
    source_offset: usize,
) {
    // 按显式换行拆分内容，保证真实字体度量也不会把换行当成字形。
    let mut remaining = content;
    // 保存当前逻辑行相对于 segment 起点已消费的字符数量。
    let mut consumed_chars = 0usize;
    // 循环消费每个逻辑行并在换行处刷新共享行缓存。
    loop {
        // 只在当前剩余内容含换行时切出下一行。
        // 使用共享辅助整体消费 CRLF、CR 或 LF。
        let (line, next) = split_once_mandatory(remaining);
        // 计算当前逻辑行在完整富文本源中的字符起点。
        let line_source_offset = source_offset + consumed_chars;
        // 使用真实字体 advance 布局当前逻辑行。
        layout_text_content_real_line(
            line,
            fs,
            color,
            bg_color,
            is_link,
            link_url,
            seg_idx,
            max_width,
            seg_line_h,
            lines,
            glyphs,
            current_x,
            max_line_w,
            font_service,
            font,
            // 传入跨 segment 共享的断行表。
            breaks,
            // 传入当前逻辑行全局字符起点。
            line_source_offset,
        );
        // 没有后续换行时当前段布局完成。
        let Some(next) = next else {
            // 退出循环并保留最后一行的当前游标。
            break;
        };
        // 显式换行前先结算当前行的最大宽度。
        *max_line_w = (*max_line_w).max(*current_x);
        // 使用当前行实际高度刷新行列表。
        flush_line(lines, glyphs, (*current_line_h).max(seg_line_h));
        // 换行后从行首重新开始布局。
        *current_x = 0.0;
        // 新行至少保留默认行高和当前段行高中的较大值。
        *current_line_h = default_line_h.max(seg_line_h);
        // 计算本轮逻辑行和强制换行分隔符共同消费的字符数量。
        consumed_chars += remaining.chars().count() - next.chars().count();
        // 继续处理换行后的剩余内容。
        remaining = next;
    }
}

/// 使用真实字体 advance 布局单个逻辑行。
#[allow(clippy::too_many_arguments)]
fn layout_text_content_real_line(
    content: &str,
    fs: f32,
    color: Color,
    bg_color: Option<Color>,
    is_link: bool,
    link_url: Option<&str>,
    seg_idx: usize,
    max_width: f32,
    seg_line_h: f32,
    lines: &mut Vec<LayoutLine>,
    glyphs: &mut Vec<LayoutGlyph>,
    current_x: &mut f32,
    max_line_w: &mut f32,
    font_service: &FontService,
    font: &FontHandle,
    breaks: &LineBreakMap,
    source_offset: usize,
) {
    let advances = real_char_advances(font_service, font, content, fs);
    let chars: Vec<char> = content.chars().collect();
    let shared_url: Option<std::sync::Arc<str>> = link_url.map(std::sync::Arc::from);

    let mut start = 0usize;
    let total = chars.len();

    while start < total {
        // 至少把当前字符纳入 UAX 原子片段。
        let mut end = start + 1;
        // 扩展到下一个标准允许或强制断行边界。
        while end < total && !breaks.allows_at(source_offset + end) {
            // 继续保留禁止断行的后继字符。
            end += 1;
        }

        let word_advances = &advances[start..end.min(advances.len())];
        let word_w: f32 = word_advances.iter().sum();

        if breaks.allows_at(source_offset + start)
            && *current_x + word_w > max_width
            && !glyphs.is_empty()
        {
            *max_line_w = (*max_line_w).max(*current_x);
            flush_line(lines, glyphs, seg_line_h);
            *current_x = 0.0;
        }

        if *current_x + word_w > max_width {
            let mut word_x = *current_x;
            for (relative_index, (&ch, &cw)) in chars[start..end]
                // 遍历片段字符与真实 advance。
                .iter()
                // 保持字符和宽度一一对应。
                .zip(word_advances)
                // 保留片段内逻辑字符索引。
                .enumerate()
            {
                // 零 advance 的 ligature 后继字符不能形成紧急断点。
                let emergency_break = cw > 0.0
                    // 共享表必须允许当前长单词边界。
                    && breaks.emergency_allows_at(source_offset + start + relative_index);
                if word_x + cw > max_width && word_x > 0.0 && !glyphs.is_empty() && emergency_break
                {
                    *max_line_w = (*max_line_w).max(word_x);
                    flush_line(lines, glyphs, seg_line_h);
                    word_x = 0.0;
                }
                glyphs.push(LayoutGlyph {
                    segment_idx: seg_idx,
                    // 直接保存完整源字符索引，保留 CRLF 与跨样式段偏移。
                    global_char_idx: source_offset + start + relative_index,
                    ch,
                    x: word_x,
                    width: cw,
                    font_size: fs,
                    color,
                    bg_color,
                    is_link,
                    link_url: shared_url.clone(),
                });
                word_x += cw;
            }
            *current_x = word_x;
        } else {
            for (relative_index, (&ch, &cw)) in chars[start..end]
                // 遍历片段字符与真实 advance。
                .iter()
                // 保持字符和宽度一一对应。
                .zip(word_advances)
                // 保留片段内逻辑字符索引。
                .enumerate()
            {
                glyphs.push(LayoutGlyph {
                    segment_idx: seg_idx,
                    // 直接保存完整源字符索引，保留 CRLF 与跨样式段偏移。
                    global_char_idx: source_offset + start + relative_index,
                    ch,
                    x: *current_x,
                    width: cw,
                    font_size: fs,
                    color,
                    bg_color,
                    is_link,
                    link_url: shared_url.clone(),
                });
                *current_x += cw;
            }
        }
        start = end;
    }
}
