// 复用富文本的段类型定义。
use super::RichTextSegment;
// 复用组件根注入的语义调色板值。
use super::RichTextPalette;
// 复用独立的布局字形、行与代码复制区域类型。
use super::layout_types::{LayoutGlyph, LayoutLine};
// 复用独立的估算字符宽度、行刷新与完整逻辑源拼接。
use super::layout_metrics::{char_width, flush_line, source_text};
// 引入富文本视觉 run 重排与定位。
use super::bidi_layout::reorder_lines;
// 复用独立的真实字体逐字符度量逻辑。
use super::shaped_advance::real_char_advances;
// 复用主题分隔线的独立布局行组件。
use super::thematic_break;
// 复用图片原子几何和运行时尺寸状态。
use super::inline_image::InlineImageStates;
use super::presentation::{RICH_TEXT_VISUAL, RichTextMetricsVisual};
// 图片能力开启时调用原子布局实现。
#[cfg(feature = "image-codecs")]
use super::inline_image;
// 测试直接验证 shaping cluster advance 映射。
#[cfg(test)]
use super::shaped_advance::measured_advance_for_char;
use crate::draw::resources::font::font_service::FontService;
// 引入共享段落级 UAX #9 分析。
use crate::draw::resources::font::bidi::BidiAnalysis;
// 让估算与真实富文本布局共享 UAX #14 断行边界和强制换行切分。
use crate::draw::resources::font::line_break::{LineBreakMap, split_once_mandatory};
// 测试使用后端字形构造稀疏索引和 kerning 场景。
#[cfg(test)]
use crate::draw::resources::font::text_backend::PositionedGlyph;
use crate::draw::{Color, FontHandle, Transform};
use crate::ui::widget_runtime::paint_context::PaintContext;

// 旧测试路径继续从布局实现模块读取无状态估算入口。
#[cfg(test)]
pub(crate) use super::rich_text_layout_entry::layout_rich_text;
// 绘制一个连续富文本 run，并在需要时应用粗体和斜体样式。
pub(crate) fn draw_rich_text_run(
    ctx: &mut PaintContext,
    content: &str,
    pos: crate::core::Point,
    color: Color,
    font_size: f32,
    segment: Option<&RichTextSegment>,
    metrics: RichTextMetricsVisual,
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
        ctx.concat_transform(italic_transform_with_shear(pos.y, metrics.italic_shear));
    }
    // 使用既有字体服务绘制原始 run，保持测量和光栅化入口一致。
    ctx.draw_text(content, pos, color, font_size);
    // 沿用现有粗体策略，避免改变已验证的字宽与字体选择。
    if bold {
        // 通过轻微水平偏移叠加字形形成粗体视觉效果。
        ctx.draw_text(
            content,
            crate::core::Point::new(pos.x + metrics.bold_offset, pos.y),
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
#[cfg(test)]
pub(crate) fn italic_transform(pivot_y: f32) -> Transform {
    italic_transform_with_shear(pivot_y, RICH_TEXT_VISUAL.metrics.italic_shear)
}

// 使用 UIX 声明的倾斜比例返回局部斜体仿射变换。
fn italic_transform_with_shear(pivot_y: f32, shear: f32) -> Transform {
    // 非有限坐标回退到原点，避免把无效状态写入绘制命令。
    let pivot_y = if pivot_y.is_finite() { pivot_y } else { 0.0 };
    // 先移到局部轴，再剪切，最后移回原坐标系。
    Transform::translate(0.0, pivot_y)
        .concat(Transform {
            m: [1.0, shear, 0.0, 0.0, 1.0, 0.0],
        })
        .concat(Transform::translate(0.0, -pivot_y))
}

// 把布局内部回归测试放入独立文件，维持生产组件规模上限。
#[cfg(test)]
// 指向布局模块的私有测试实现。
#[path = "../../../../../tests/unit/ui/widgets/other/rich_text/rich_text_layout_tests.rs"]
mod tests;

// 估算布局（不依赖 FontService）。
// 兼容内部测试与无视觉调用方，复用 UIX 唯一视觉表。
pub(crate) fn layout_rich_text_with_images(
    segments: &[RichTextSegment],
    max_width: f32,
    default_font_size: f32,
    palette: RichTextPalette,
    image_states: &InlineImageStates,
) -> (Vec<LayoutLine>, f32, f32) {
    layout_rich_text_with_images_visual(
        segments,
        max_width,
        default_font_size,
        palette,
        image_states,
        RICH_TEXT_VISUAL.metrics,
    )
}

// 使用调用方图片资源状态执行估算布局。
pub(crate) fn layout_rich_text_with_images_visual(
    // 接收公开段列表。
    segments: &[RichTextSegment],
    // 接收最大行宽。
    max_width: f32,
    // 接收默认字号。
    default_font_size: f32,
    // 接收调用方解析完成的语义调色板。
    palette: RichTextPalette,
    // 接收当前图片固有尺寸状态。
    image_states: &InlineImageStates,
    // 接收 UIX 声明的排版比例。
    metrics: RichTextMetricsVisual,
) -> (Vec<LayoutLine>, f32, f32) {
    // 图片能力关闭时显式消费空状态表参数。
    #[cfg(not(feature = "image-codecs"))]
    let _ = image_states;
    // 以完整富文本源生成跨样式段共享的 UAX #14 边界。
    let full_source = source_text(segments);
    // 同一边界表贯穿 Text、Code 与 Link 段。
    let breaks = LineBreakMap::new(&full_source);
    let mut lines: Vec<LayoutLine> = Vec::new();
    let mut current_line_glyphs: Vec<LayoutGlyph> = Vec::new();
    let mut current_x: f32 = 0.0;
    let line_height_factor = metrics.line_height_factor;
    let default_line_h = default_font_size * line_height_factor;
    let mut max_line_w: f32 = 0.0;
    // 保存当前 segment 在完整逻辑源中的字符起点。
    let mut source_offset = 0usize;
    // 记录主题分隔线已经消费其行盒，紧随 NewLine 只保留逻辑换行。
    let mut thematic_break_awaiting_newline = false;

    for (seg_idx, segment) in segments.iter().enumerate() {
        // 只让紧邻主题分隔线的换行复用该块已经占用的行盒。
        let follows_thematic_break = std::mem::take(&mut thematic_break_awaiting_newline);
        match segment {
            RichTextSegment::ThematicBreak => {
                thematic_break::push_layout_line(
                    &mut lines,
                    &mut current_line_glyphs,
                    default_line_h,
                    metrics,
                );
                current_x = 0.0;
                max_line_w = max_line_w.max(thematic_break::layout_width(max_width));
                thematic_break_awaiting_newline = true;
            }
            RichTextSegment::NewLine if follows_thematic_break => {
                source_offset += 1;
            }
            RichTextSegment::NewLine => {
                max_line_w = max_line_w.max(current_x);
                // 以默认行高为下限，并由刷新入口保留当前行最大字形行高。
                flush_line(
                    &mut lines,
                    &mut current_line_glyphs,
                    default_line_h,
                    metrics,
                );
                current_x = 0.0;
                // 显式换行在完整逻辑源中占一个字符位置。
                source_offset += 1;
            }
            RichTextSegment::Text { content, style } => {
                let fs = style.resolved_font_size(default_font_size);
                let color = style.resolved_color(palette.default_text);
                let bg = style.bg_color;
                let seg_line_h = fs * line_height_factor;
                layout_text_content(
                    content,
                    fs,
                    color,
                    bg,
                    false,
                    seg_idx,
                    max_width,
                    seg_line_h,
                    default_line_h,
                    &mut lines,
                    &mut current_line_glyphs,
                    &mut current_x,
                    &mut max_line_w,
                    // 传入完整源断行表。
                    &breaks,
                    // 传入当前文本段全局字符起点。
                    source_offset,
                    // 传入 UIX 声明的排版比例。
                    metrics,
                );
                // 推进到下一 segment 的全局字符起点。
                source_offset += content.chars().count();
            }
            RichTextSegment::Code { content } => {
                let fs = default_font_size * metrics.code_font_scale;
                // 代码文本颜色由组件根从当前主题作用域注入。
                let color = palette.code_text;
                // 代码背景颜色同样只消费解析后的语义值。
                let bg = palette.code_background;
                let seg_line_h = fs * line_height_factor;
                layout_text_content(
                    content,
                    fs,
                    color,
                    Some(bg),
                    false,
                    seg_idx,
                    max_width,
                    seg_line_h,
                    default_line_h,
                    &mut lines,
                    &mut current_line_glyphs,
                    &mut current_x,
                    &mut max_line_w,
                    // 传入完整源断行表。
                    &breaks,
                    // 传入当前代码段全局字符起点。
                    source_offset,
                    // 传入 UIX 声明的排版比例。
                    metrics,
                );
                // 推进到下一 segment 的全局字符起点。
                source_offset += content.chars().count();
            }
            RichTextSegment::Link { content, .. } => {
                let fs = default_font_size;
                // 链接颜色由组件根从当前主题作用域注入。
                let color = palette.link;
                let seg_line_h = fs * line_height_factor;
                layout_text_content(
                    content,
                    fs,
                    color,
                    None,
                    true,
                    seg_idx,
                    max_width,
                    seg_line_h,
                    default_line_h,
                    &mut lines,
                    &mut current_line_glyphs,
                    &mut current_x,
                    &mut max_line_w,
                    // 传入完整源断行表。
                    &breaks,
                    // 传入当前链接段全局字符起点。
                    source_offset,
                    // 传入 UIX 声明的排版比例。
                    metrics,
                );
                // 推进到下一 segment 的全局字符起点。
                source_offset += content.chars().count();
            }
            // 图片能力开启时把公开图片段作为单一原子替换对象布局。
            #[cfg(feature = "image-codecs")]
            RichTextSegment::Image {
                alt, width, height, ..
            } => {
                // 使用共享图片几何入口保证估算和真实布局一致。
                inline_image::push_layout_glyph(
                    // 保存公开段索引。
                    seg_idx,
                    // 保存 alt 的逻辑起点。
                    source_offset,
                    // 图片逻辑跨度等于 alt 字符数。
                    alt.chars().count(),
                    // 传递可选宽度覆盖。
                    *width,
                    // 传递可选高度覆盖。
                    *height,
                    // 读取组件当前资源状态。
                    image_states.get(&seg_idx),
                    // 传递行宽约束。
                    max_width,
                    // 传递加载前占位行高。
                    default_line_h,
                    // 传递默认颜色供共享字形字段初始化。
                    palette.default_text,
                    // 传递 UIX 声明的行高比例。
                    metrics,
                    // 更新视觉行列表。
                    &mut lines,
                    // 更新当前行原子列表。
                    &mut current_line_glyphs,
                    // 更新水平游标。
                    &mut current_x,
                    // 更新最大行宽。
                    &mut max_line_w,
                );
                // 推进完整 alt 逻辑跨度。
                source_offset += alt.chars().count();
            }
        }
    }

    max_line_w = max_line_w.max(current_x);
    if !current_line_glyphs.is_empty() || lines.is_empty() {
        // 结算末行时不复用前序行状态，由字形几何决定实际行高。
        flush_line(
            &mut lines,
            &mut current_line_glyphs,
            default_line_h,
            metrics,
        );
    }

    // 使用完整逻辑源对已完成 UAX #14 折行的行应用段落级 UAX #9。
    max_line_w = max_line_w.max(reorder_lines(&mut lines, &BidiAnalysis::new(&full_source)));

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
    seg_idx: usize,
    max_width: f32,
    seg_line_h: f32,
    default_line_h: f32,
    lines: &mut Vec<LayoutLine>,
    glyphs: &mut Vec<LayoutGlyph>,
    current_x: &mut f32,
    max_line_w: &mut f32,
    breaks: &LineBreakMap,
    source_offset: usize,
    metrics: RichTextMetricsVisual,
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
        // 当前逻辑行字符只统计一次，避免反复扫描剩余后缀。
        let line_char_count = line.chars().count();
        // 计算当前逻辑行在完整富文本源中的字符起点。
        let line_source_offset = source_offset + consumed_chars;
        // 使用原有空白 token 逻辑布局当前行。
        layout_text_content_line(
            line,
            fs,
            color,
            bg_color,
            is_link,
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
            // 传入 UIX 声明的排版比例。
            metrics,
        );
        // 没有后续换行时当前段布局完成。
        let Some(next) = next else {
            // 退出循环并保留最后一行的当前游标。
            break;
        };
        // 显式换行前先结算当前行的最大宽度。
        *max_line_w = (*max_line_w).max(*current_x);
        // 以默认与当前段行高为下限，并保留同一行前序段的更大字号。
        flush_line(lines, glyphs, default_line_h.max(seg_line_h), metrics);
        // 换行后从行首重新开始布局。
        *current_x = 0.0;
        // CR、LF 与 CRLF 都是 ASCII，字节长度等于其逻辑字符数量。
        let separator_char_count = remaining.len() - line.len() - next.len();
        // 只累加本轮新增的逻辑行与强制换行分隔符。
        consumed_chars += line_char_count + separator_char_count;
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
    seg_idx: usize,
    max_width: f32,
    seg_line_h: f32,
    lines: &mut Vec<LayoutLine>,
    glyphs: &mut Vec<LayoutGlyph>,
    current_x: &mut f32,
    max_line_w: &mut f32,
    breaks: &LineBreakMap,
    source_offset: usize,
    metrics: RichTextMetricsVisual,
) {
    // 以 UTF-8 字节游标遍历原字符串，保留字符索引而不创建临时 Vec。
    let mut chars = content.char_indices().peekable();
    // 从首个字符开始消费相邻 UAX 机会之间的原子片段。
    let mut start = 0usize;
    // 逐段消费直到完整逻辑行结束。
    while let Some((start_byte, _)) = chars.next() {
        // 至少把当前字符纳入片段。
        let mut end = start + 1;
        // 记录当前片段的排他字节末端，供后续无分配切片迭代。
        let mut end_byte = chars.peek().map(|(byte, _)| *byte).unwrap_or(content.len());
        // 扩展到下一个标准允许或强制边界。
        while chars.peek().is_some() && !breaks.allows_at(source_offset + end) {
            // 消费仍属于当前原子片段的后继字符。
            chars.next();
            end += 1;
            end_byte = chars.peek().map(|(byte, _)| *byte).unwrap_or(content.len());
        }
        // 计算当前 UAX 原子片段的估算宽度。
        let token_w = content[start_byte..end_byte]
            // 遍历片段字符。
            .chars()
            // 使用共享字符宽度估算。
            .map(|ch| char_width(fs, ch, metrics))
            // 聚合完整片段宽度。
            .sum::<f32>();

        if breaks.allows_at(source_offset + start)
            && *current_x + token_w > max_width
            && !glyphs.is_empty()
        {
            *max_line_w = (*max_line_w).max(*current_x);
            flush_line(lines, glyphs, seg_line_h, metrics);
            *current_x = 0.0;
        }

        if *current_x + token_w > max_width {
            let mut word_chars_x = *current_x;
            for (relative_index, ch) in content[start_byte..end_byte].chars().enumerate() {
                let cw = char_width(fs, ch, metrics);
                // 只有长字母数字词的紧急边界可以绕过标准 UAX 机会。
                let emergency_break =
                    breaks.emergency_allows_at(source_offset + start + relative_index);
                if word_chars_x + cw > max_width
                    && word_chars_x > 0.0
                    && !glyphs.is_empty()
                    && emergency_break
                {
                    *max_line_w = (*max_line_w).max(word_chars_x);
                    flush_line(lines, glyphs, seg_line_h, metrics);
                    word_chars_x = 0.0;
                }
                glyphs.push(LayoutGlyph {
                    // 普通估算字符进入文本绘制路径。
                    kind: super::LayoutGlyphKind::Text,
                    segment_idx: seg_idx,
                    // 直接保存完整源字符索引，保留 CRLF 与跨样式段偏移。
                    global_char_idx: source_offset + start + relative_index,
                    // 单个 Unicode 标量覆盖一个逻辑字符位置。
                    source_char_len: 1,
                    // 视觉行完成后由共享 UAX #9 分析回填。
                    bidi_level: 0,
                    ch,
                    x: word_chars_x,
                    width: cw,
                    font_size: fs,
                    color,
                    bg_color,
                    is_link,
                });
                word_chars_x += cw;
            }
            *current_x = word_chars_x;
        } else {
            for (relative_index, ch) in content[start_byte..end_byte].chars().enumerate() {
                let cw = char_width(fs, ch, metrics);
                glyphs.push(LayoutGlyph {
                    // 普通估算字符进入文本绘制路径。
                    kind: super::LayoutGlyphKind::Text,
                    segment_idx: seg_idx,
                    // 直接保存完整源字符索引，保留 CRLF 与跨样式段偏移。
                    global_char_idx: source_offset + start + relative_index,
                    // 单个 Unicode 标量覆盖一个逻辑字符位置。
                    source_char_len: 1,
                    // 视觉行完成后由共享 UAX #9 分析回填。
                    bidi_level: 0,
                    ch,
                    x: *current_x,
                    width: cw,
                    font_size: fs,
                    color,
                    bg_color,
                    is_link,
                });
                *current_x += cw;
            }
        }
        // 从当前 UAX 边界继续处理后续片段。
        start = end;
    }
}

// 真实字体度量布局（用于 render 阶段）。

// 兼容内部测试与无视觉调用方，复用 UIX 唯一视觉表。
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub(crate) fn layout_rich_text_real_with_images(
    segments: &[RichTextSegment],
    max_width: f32,
    default_font_size: f32,
    palette: RichTextPalette,
    font_service: &FontService,
    font: &FontHandle,
    image_states: &InlineImageStates,
) -> (Vec<LayoutLine>, f32, f32) {
    layout_rich_text_real_with_images_visual(
        segments,
        max_width,
        default_font_size,
        palette,
        font_service,
        font,
        image_states,
        RICH_TEXT_VISUAL.metrics,
    )
}

// 使用真实字体与调用方图片状态执行布局。
#[allow(clippy::too_many_arguments)]
pub(crate) fn layout_rich_text_real_with_images_visual(
    // 接收公开段列表。
    segments: &[RichTextSegment],
    // 接收最大行宽。
    max_width: f32,
    // 接收默认字号。
    default_font_size: f32,
    // 接收调用方解析完成的语义调色板。
    palette: RichTextPalette,
    // 接收字体服务。
    font_service: &FontService,
    // 接收字体句柄。
    font: &FontHandle,
    // 接收当前图片固有尺寸状态。
    image_states: &InlineImageStates,
    // 接收 UIX 声明的排版比例。
    metrics: RichTextMetricsVisual,
) -> (Vec<LayoutLine>, f32, f32) {
    // 图片能力关闭时显式消费空状态表参数。
    #[cfg(not(feature = "image-codecs"))]
    let _ = image_states;
    // 以完整富文本源生成跨样式段共享的 UAX #14 边界。
    let full_source = source_text(segments);
    // 同一边界表贯穿 Text、Code 与 Link 的真实字体路径。
    let breaks = LineBreakMap::new(&full_source);
    let mut lines: Vec<LayoutLine> = Vec::new();
    let mut current_line_glyphs: Vec<LayoutGlyph> = Vec::new();
    let mut current_x: f32 = 0.0;
    let line_height_factor = metrics.line_height_factor;
    let default_line_h = default_font_size * line_height_factor;
    let mut max_line_w: f32 = 0.0;
    // 保存当前 segment 在完整逻辑源中的字符起点。
    let mut source_offset = 0usize;
    // 真实字体路径沿用相同的分隔线换行消费状态。
    let mut thematic_break_awaiting_newline = false;

    for (seg_idx, segment) in segments.iter().enumerate() {
        // 真实字体路径同样只复用紧邻分隔线的源码换行。
        let follows_thematic_break = std::mem::take(&mut thematic_break_awaiting_newline);
        match segment {
            RichTextSegment::ThematicBreak => {
                thematic_break::push_layout_line(
                    &mut lines,
                    &mut current_line_glyphs,
                    default_line_h,
                    metrics,
                );
                current_x = 0.0;
                max_line_w = max_line_w.max(thematic_break::layout_width(max_width));
                thematic_break_awaiting_newline = true;
            }
            RichTextSegment::NewLine if follows_thematic_break => {
                source_offset += 1;
            }
            RichTextSegment::NewLine => {
                max_line_w = max_line_w.max(current_x);
                // 真实布局同样以默认行高为下限，并扫描当前行实际字形。
                flush_line(
                    &mut lines,
                    &mut current_line_glyphs,
                    default_line_h,
                    metrics,
                );
                current_x = 0.0;
                // 显式换行在完整逻辑源中占一个字符位置。
                source_offset += 1;
            }
            RichTextSegment::Text { content, style } => {
                let fs = style.resolved_font_size(default_font_size);
                let color = style.resolved_color(palette.default_text);
                let bg = style.bg_color;
                let seg_line_h = fs * line_height_factor;
                layout_text_content_real(
                    content,
                    fs,
                    color,
                    bg,
                    false,
                    seg_idx,
                    max_width,
                    seg_line_h,
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
                    // 传入 UIX 声明的排版比例。
                    metrics,
                );
                // 推进到下一 segment 的全局字符起点。
                source_offset += content.chars().count();
            }
            RichTextSegment::Code { content } => {
                let fs = default_font_size * metrics.code_font_scale;
                // 代码文本颜色由组件根从当前主题作用域注入。
                let color = palette.code_text;
                // 代码背景颜色同样只消费解析后的语义值。
                let bg = palette.code_background;
                let seg_line_h = fs * line_height_factor;
                layout_text_content_real(
                    content,
                    fs,
                    color,
                    Some(bg),
                    false,
                    seg_idx,
                    max_width,
                    seg_line_h,
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
                    // 传入 UIX 声明的排版比例。
                    metrics,
                );
                // 推进到下一 segment 的全局字符起点。
                source_offset += content.chars().count();
            }
            RichTextSegment::Link { content, .. } => {
                let fs = default_font_size;
                // 链接颜色由组件根从当前主题作用域注入。
                let color = palette.link;
                let seg_line_h = fs * line_height_factor;
                layout_text_content_real(
                    content,
                    fs,
                    color,
                    None,
                    true,
                    seg_idx,
                    max_width,
                    seg_line_h,
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
                    // 传入 UIX 声明的排版比例。
                    metrics,
                );
                // 推进到下一 segment 的全局字符起点。
                source_offset += content.chars().count();
            }
            // 真实字体路径仍复用相同的图片原子几何。
            #[cfg(feature = "image-codecs")]
            RichTextSegment::Image {
                alt, width, height, ..
            } => {
                // 图片不经过字体 shaping，只登记替换对象几何。
                inline_image::push_layout_glyph(
                    // 保存公开段索引。
                    seg_idx,
                    // 保存 alt 的逻辑起点。
                    source_offset,
                    // 图片逻辑跨度等于 alt 字符数。
                    alt.chars().count(),
                    // 传递可选宽度覆盖。
                    *width,
                    // 传递可选高度覆盖。
                    *height,
                    // 读取组件当前资源状态。
                    image_states.get(&seg_idx),
                    // 传递行宽约束。
                    max_width,
                    // 传递加载前占位行高。
                    default_line_h,
                    // 传递默认颜色供共享字段初始化。
                    palette.default_text,
                    // 传递 UIX 声明的行高比例。
                    metrics,
                    // 更新视觉行列表。
                    &mut lines,
                    // 更新当前行原子列表。
                    &mut current_line_glyphs,
                    // 更新水平游标。
                    &mut current_x,
                    // 更新最大行宽。
                    &mut max_line_w,
                );
                // 推进完整 alt 逻辑跨度。
                source_offset += alt.chars().count();
            }
        }
    }

    max_line_w = max_line_w.max(current_x);
    if !current_line_glyphs.is_empty() || lines.is_empty() {
        // 真实布局末行只使用本行字形决定放大后的行盒高度。
        flush_line(
            &mut lines,
            &mut current_line_glyphs,
            default_line_h,
            metrics,
        );
    }

    // 真实字体路径与估算路径共享同一段落级 UAX #9 视觉 run 数据。
    max_line_w = max_line_w.max(reorder_lines(&mut lines, &BidiAnalysis::new(&full_source)));

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
    seg_idx: usize,
    max_width: f32,
    seg_line_h: f32,
    default_line_h: f32,
    lines: &mut Vec<LayoutLine>,
    glyphs: &mut Vec<LayoutGlyph>,
    current_x: &mut f32,
    max_line_w: &mut f32,
    font_service: &FontService,
    font: &FontHandle,
    breaks: &LineBreakMap,
    source_offset: usize,
    metrics: RichTextMetricsVisual,
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
        // 当前逻辑行字符只统计一次，避免反复扫描剩余后缀。
        let line_char_count = line.chars().count();
        // 计算当前逻辑行在完整富文本源中的字符起点。
        let line_source_offset = source_offset + consumed_chars;
        // 使用真实字体 advance 布局当前逻辑行。
        layout_text_content_real_line(
            line,
            fs,
            color,
            bg_color,
            is_link,
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
            // 传入 UIX 声明的排版比例。
            metrics,
        );
        // 没有后续换行时当前段布局完成。
        let Some(next) = next else {
            // 退出循环并保留最后一行的当前游标。
            break;
        };
        // 显式换行前先结算当前行的最大宽度。
        *max_line_w = (*max_line_w).max(*current_x);
        // 以默认与当前段行高为下限，并保留同一行前序段的更大字号。
        flush_line(lines, glyphs, default_line_h.max(seg_line_h), metrics);
        // 换行后从行首重新开始布局。
        *current_x = 0.0;
        // CR、LF 与 CRLF 都是 ASCII，字节长度等于其逻辑字符数量。
        let separator_char_count = remaining.len() - line.len() - next.len();
        // 只累加本轮新增的逻辑行与强制换行分隔符。
        consumed_chars += line_char_count + separator_char_count;
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
    metrics: RichTextMetricsVisual,
) {
    let advances = real_char_advances(font_service, font, content, fs, metrics);
    // 以 UTF-8 字节游标遍历原字符串，避免为真实 advance 再复制字符数组。
    let mut chars = content.char_indices().peekable();

    let mut start = 0usize;

    while let Some((start_byte, _)) = chars.next() {
        // 至少把当前字符纳入 UAX 原子片段。
        let mut end = start + 1;
        // 记录当前片段的排他字节末端，供字符与 advance 无分配配对。
        let mut end_byte = chars.peek().map(|(byte, _)| *byte).unwrap_or(content.len());
        // 扩展到下一个标准允许或强制断行边界。
        while chars.peek().is_some() && !breaks.allows_at(source_offset + end) {
            // 消费仍属于当前原子片段的后继字符。
            chars.next();
            end += 1;
            end_byte = chars.peek().map(|(byte, _)| *byte).unwrap_or(content.len());
        }

        let word_advances = &advances[start..end.min(advances.len())];
        let word_w: f32 = word_advances.iter().sum();

        if breaks.allows_at(source_offset + start)
            && *current_x + word_w > max_width
            && !glyphs.is_empty()
        {
            *max_line_w = (*max_line_w).max(*current_x);
            flush_line(lines, glyphs, seg_line_h, metrics);
            *current_x = 0.0;
        }

        if *current_x + word_w > max_width {
            let mut word_x = *current_x;
            for (relative_index, (ch, &cw)) in content[start_byte..end_byte]
                .chars()
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
                    flush_line(lines, glyphs, seg_line_h, metrics);
                    word_x = 0.0;
                }
                glyphs.push(LayoutGlyph {
                    // 真实字体字符仍属于文本绘制路径。
                    kind: super::LayoutGlyphKind::Text,
                    segment_idx: seg_idx,
                    // 直接保存完整源字符索引，保留 CRLF 与跨样式段偏移。
                    global_char_idx: source_offset + start + relative_index,
                    // 单个 Unicode 标量覆盖一个逻辑字符位置。
                    source_char_len: 1,
                    // 视觉行完成后由共享 UAX #9 分析回填。
                    bidi_level: 0,
                    ch,
                    x: word_x,
                    width: cw,
                    font_size: fs,
                    color,
                    bg_color,
                    is_link,
                });
                word_x += cw;
            }
            *current_x = word_x;
        } else {
            for (relative_index, (ch, &cw)) in content[start_byte..end_byte]
                .chars()
                // 保持字符和宽度一一对应。
                .zip(word_advances)
                // 保留片段内逻辑字符索引。
                .enumerate()
            {
                glyphs.push(LayoutGlyph {
                    // 真实字体字符仍属于文本绘制路径。
                    kind: super::LayoutGlyphKind::Text,
                    segment_idx: seg_idx,
                    // 直接保存完整源字符索引，保留 CRLF 与跨样式段偏移。
                    global_char_idx: source_offset + start + relative_index,
                    // 单个 Unicode 标量覆盖一个逻辑字符位置。
                    source_char_len: 1,
                    // 视觉行完成后由共享 UAX #9 分析回填。
                    bidi_level: 0,
                    ch,
                    x: *current_x,
                    width: cw,
                    font_size: fs,
                    color,
                    bg_color,
                    is_link,
                });
                *current_x += cw;
            }
        }
        start = end;
    }
}
