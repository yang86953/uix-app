//! 富文本布局引擎模块。
//!
//! 提供字符级文本布局、断行、字形缓存等功能。
//! 从 `rich_text.rs` 拆分出来以遵守 900 行文件限制。

// 复用富文本的段类型定义。
use super::RichTextSegment;
use crate::core::Rect;
use crate::draw::resources::font::font_service::FontService;
// 读取真实字体布局选项与后端返回的源字符索引字形。
use crate::draw::resources::font::text_backend::{PositionedGlyph, TextLayoutOptions};
use crate::draw::{Color, FontHandle, Transform};
use crate::ui::component::paint_context::PaintContext;

// ══════════════════════════════════════════════════════════════════
// 布局类型
// ══════════════════════════════════════════════════════════════════

/// 布局后的字形（带样式信息）
#[derive(Debug, Clone)]
pub(crate) struct LayoutGlyph {
    pub segment_idx: usize,
    pub global_char_idx: usize,
    pub ch: char,
    pub x: f32,
    pub width: f32,
    pub font_size: f32,
    pub color: Color,
    pub bg_color: Option<Color>,
    pub is_link: bool,
    /// 链接 URL（使用 Arc 共享，避免每个字形都分配新 String）
    pub link_url: Option<std::sync::Arc<str>>,
}

/// 布局后的行
#[derive(Debug, Clone)]
pub(crate) struct LayoutLine {
    pub y: f32,
    pub height: f32,
    pub glyphs: Vec<LayoutGlyph>,
}

/// 代码块复制按钮区域
#[derive(Debug, Clone)]
pub(crate) struct CodeCopyRegion {
    pub rect: Rect,
    pub segment_idx: usize,
}

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
                font: FontHandle::new(0),
            },
            PositionedGlyph {
                x: 10.0,
                y: 0.0,
                width: 20.0,
                height: 12.0,
                glyph_id: 2,
                char_index: 2,
                font: FontHandle::new(0),
            },
            PositionedGlyph {
                x: 30.0,
                y: 0.0,
                width: 0.0,
                height: 12.0,
                glyph_id: 3,
                char_index: 3,
                font: FontHandle::new(0),
            },
        ];
        // 索引 2 应读取第二个字形的真实宽度，而不是索引 1 的槽位。
        assert_eq!(measured_advance_for_char(&glyphs, 2, 7.0), 20.0);
        // 后端明确返回的零宽字形不能被误回退为可见宽度。
        assert_eq!(measured_advance_for_char(&glyphs, 3, 7.0), 0.0);
        // 没有字形的索引应回退到估算宽度。
        assert_eq!(measured_advance_for_char(&glyphs, 1, 7.0), 7.0);
    }
}

// ══════════════════════════════════════════════════════════════════
// 字符宽度估算
// ══════════════════════════════════════════════════════════════════

/// 估算字符宽度（px），基于字体大小的比例
fn char_width(fs: f32, ch: char) -> f32 {
    match ch {
        ' ' => fs * 0.35,
        '\t' => fs * 2.0,
        'm' | 'M' | 'W' | 'w' => fs * 0.7,
        'i' | 'I' | 'l' | '1' | '.' | ',' | ':' | ';' | '\'' => fs * 0.3,
        c if is_cjk(c) => fs * 1.0,
        c if ('\u{3000}'..='\u{303f}').contains(&c) => fs * 0.9,
        _ => fs * 0.55,
    }
}

/// 判断字符是否属于 CJK（可用于断行）
fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{4E00}'..='\u{9FFF}'
        | '\u{3400}'..='\u{4DBF}'
        | '\u{20000}'..='\u{2A6DF}'
        | '\u{3040}'..='\u{309F}'
        | '\u{30A0}'..='\u{30FF}'
        | '\u{FF66}'..='\u{FF9F}'
        | '\u{AC00}'..='\u{D7AF}'
        | '\u{1100}'..='\u{11FF}'
        | '\u{1200}'..='\u{137F}'
    )
}

/// 估算文本宽度
pub(crate) fn text_width(text: &str, fs: f32) -> f32 {
    text.chars().map(|c| char_width(fs, c)).sum()
}

/// 刷新当前行到行列表
pub(crate) fn flush_line(
    lines: &mut Vec<LayoutLine>,
    glyphs: &mut Vec<LayoutGlyph>,
    line_height: f32,
) {
    let y = lines.last().map(|l| l.y + l.height).unwrap_or(0.0);
    let g = std::mem::take(glyphs);
    lines.push(LayoutLine {
        y,
        height: line_height.max(1.0),
        glyphs: g,
    });
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
    let mut lines: Vec<LayoutLine> = Vec::new();
    let mut current_line_glyphs: Vec<LayoutGlyph> = Vec::new();
    let mut current_x: f32 = 0.0;
    let line_height_factor: f32 = 1.5;
    let default_line_h = default_font_size * line_height_factor;
    let mut current_line_h: f32 = default_line_h;
    let mut max_line_w: f32 = 0.0;

    for (seg_idx, segment) in segments.iter().enumerate() {
        match segment {
            RichTextSegment::NewLine => {
                max_line_w = max_line_w.max(current_x);
                flush_line(&mut lines, &mut current_line_glyphs, current_line_h);
                current_x = 0.0;
                current_line_h = default_line_h;
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
                );
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
                );
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
                );
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
) {
    // 按显式换行把内容拆成多个逻辑行，保证估算布局与真实布局共享换行语义。
    let mut remaining = content;
    // 循环消费当前段中的每一行，保留末尾空行的边界行为。
    loop {
        // 只在当前行存在换行时切出后续内容。
        let (line, next) = match remaining.split_once('\n') {
            // 记录当前行和换行后的剩余内容。
            Some((line, next)) => (line, Some(next)),
            // 没有换行时当前剩余内容就是最后一行。
            None => (remaining, None),
        };
        // 使用原有空白 token 逻辑布局当前行。
        layout_text_content_line(
            line, fs, color, bg_color, is_link, link_url, seg_idx, max_width, seg_line_h, lines,
            glyphs, current_x, max_line_w,
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
        // 继续处理换行后的剩余内容。
        remaining = next;
    }
}

/// 使用空白 token 和逐字符回退布局单个逻辑行。
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
) {
    let shared_url: Option<std::sync::Arc<str>> = link_url.map(std::sync::Arc::from);
    // 按所有 Unicode 空白结束逻辑 token，保持文档约定的空白断行语义。
    let tokens: Vec<&str> = content
        .split_inclusive(|ch: char| ch.is_whitespace())
        .collect();

    for token in &tokens {
        let token_w = text_width(token, fs);

        if *current_x + token_w > max_width && !glyphs.is_empty() {
            *max_line_w = (*max_line_w).max(*current_x);
            flush_line(lines, glyphs, seg_line_h);
            *current_x = 0.0;
        }

        if *current_x + token_w > max_width && glyphs.is_empty() {
            let mut word_chars_x = *current_x;
            for ch in token.chars() {
                let cw = char_width(fs, ch);
                if word_chars_x + cw > max_width && word_chars_x > 0.0 && !glyphs.is_empty() {
                    *max_line_w = (*max_line_w).max(word_chars_x);
                    flush_line(lines, glyphs, seg_line_h);
                    word_chars_x = 0.0;
                }
                glyphs.push(LayoutGlyph {
                    segment_idx: seg_idx,
                    global_char_idx: 0,
                    ch,
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
            for ch in token.chars() {
                let cw = char_width(fs, ch);
                glyphs.push(LayoutGlyph {
                    segment_idx: seg_idx,
                    global_char_idx: 0,
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
    }
}

/// 按原始 segment（含显式换行）为每个字形分配全局字符索引。
pub(crate) fn assign_global_indices(lines: &mut [LayoutLine], segments: &[RichTextSegment]) {
    let mut offsets = Vec::with_capacity(segments.len());
    let mut offset = 0;
    for segment in segments {
        offsets.push(offset);
        offset += match segment {
            RichTextSegment::Text { content, .. }
            | RichTextSegment::Code { content }
            | RichTextSegment::Link { content, .. } => content.chars().count(),
            RichTextSegment::NewLine => 1,
        };
    }
    let mut seen = vec![0; segments.len()];
    for line in lines.iter_mut() {
        for glyph in line.glyphs.iter_mut() {
            if let (Some(offset), Some(seen)) = (
                offsets.get(glyph.segment_idx),
                seen.get_mut(glyph.segment_idx),
            ) {
                glyph.global_char_idx = offset + *seen;
                *seen += 1;
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// 真实字体度量布局（用于 render 阶段）
// ══════════════════════════════════════════════════════════════════

// 根据后端字形的源字符索引读取真实 advance，避免 glyph 槽位缺失时错配宽度。
fn measured_advance_for_char(glyphs: &[PositionedGlyph], char_index: usize, fallback: f32) -> f32 {
    // 只接受有限正宽度，避免异常字体度量污染布局。
    glyphs
        .iter()
        // 后端的 char_index 对应源文本 chars() 序号，而不是 glyph 数组下标。
        .find(|glyph| glyph.char_index == char_index)
        .map(|glyph| glyph.width)
        // 缺字或异常宽度回退到估算值，同时保留后端合法的零宽字形。
        .filter(|width| width.is_finite() && *width >= 0.0)
        .unwrap_or(fallback)
}

/// 获取文本中每个字符的真实 advance 宽度
fn real_char_advances(
    font_service: &FontService,
    font: &FontHandle,
    text: &str,
    fs: f32,
) -> Vec<f32> {
    if text.is_empty() {
        return Vec::new();
    }
    let opts = TextLayoutOptions {
        font_size: fs,
        max_width: f32::MAX,
        max_height: 0.0,
        line_height: 0.0,
        word_wrap: false,
        h_align: crate::draw::HAlign::Left,
        v_align: crate::draw::VAlign::Top,
    };
    let layout = font_service.layout_text(font, text, &opts);
    let chars: Vec<char> = text.chars().collect();
    let mut advances = Vec::with_capacity(chars.len());
    for (i, ch) in chars.iter().enumerate() {
        // 当前字符没有可用 glyph 时使用估算宽度保持布局可收敛。
        let fallback = char_width(fs, *ch);
        // 按后端提供的源字符索引读取真实宽度，保留缺口后的字符对齐。
        advances.push(measured_advance_for_char(&layout.glyphs, i, fallback));
    }
    advances
}

/// 基于真实字体度量执行富文本布局
pub(crate) fn layout_rich_text_real(
    segments: &[RichTextSegment],
    max_width: f32,
    default_font_size: f32,
    default_color: Color,
    font_service: &FontService,
    font: &FontHandle,
) -> (Vec<LayoutLine>, f32, f32) {
    let mut lines: Vec<LayoutLine> = Vec::new();
    let mut current_line_glyphs: Vec<LayoutGlyph> = Vec::new();
    let mut current_x: f32 = 0.0;
    let line_height_factor: f32 = 1.5;
    let default_line_h = default_font_size * line_height_factor;
    let mut current_line_h: f32 = default_line_h;
    let mut max_line_w: f32 = 0.0;

    for (seg_idx, segment) in segments.iter().enumerate() {
        match segment {
            RichTextSegment::NewLine => {
                max_line_w = max_line_w.max(current_x);
                flush_line(&mut lines, &mut current_line_glyphs, current_line_h);
                current_x = 0.0;
                current_line_h = default_line_h;
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
                );
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
                );
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
                );
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
) {
    // 按显式换行拆分内容，保证真实字体度量也不会把换行当成字形。
    let mut remaining = content;
    // 循环消费每个逻辑行并在换行处刷新共享行缓存。
    loop {
        // 只在当前剩余内容含换行时切出下一行。
        let (line, next) = match remaining.split_once('\n') {
            // 记录当前行和换行后的剩余内容。
            Some((line, next)) => (line, Some(next)),
            // 没有换行时当前剩余内容就是最后一行。
            None => (remaining, None),
        };
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
) {
    let advances = real_char_advances(font_service, font, content, fs);
    let chars: Vec<char> = content.chars().collect();
    let shared_url: Option<std::sync::Arc<str>> = link_url.map(std::sync::Arc::from);

    let mut start = 0usize;
    let total = chars.len();

    while start < total {
        let mut end = start;
        // 从当前字符扫描到下一个 Unicode 空白，保持真实字体路径的 token 边界。
        while end < total && !chars[end].is_whitespace() {
            // 逐字符扩展当前非空白 token。
            end += 1;
        }
        // 将边界空白并入当前 token，保持源文本字符顺序不变。
        if end < total && chars[end].is_whitespace() {
            // 把当前空白字符留在当前 token 尾部，后续正文从下一个 token 开始。
            end += 1;
        }
        if end == start {
            start = end;
            continue;
        }

        let word_advances = &advances[start..end.min(advances.len())];
        let word_w: f32 = word_advances.iter().sum();

        if *current_x + word_w > max_width && !glyphs.is_empty() {
            *max_line_w = (*max_line_w).max(*current_x);
            flush_line(lines, glyphs, seg_line_h);
            *current_x = 0.0;
        }

        if *current_x + word_w > max_width && glyphs.is_empty() {
            let mut word_x = *current_x;
            for (&ch, &cw) in chars[start..end].iter().zip(word_advances) {
                if word_x + cw > max_width && word_x > 0.0 && !glyphs.is_empty() {
                    *max_line_w = (*max_line_w).max(word_x);
                    flush_line(lines, glyphs, seg_line_h);
                    word_x = 0.0;
                }
                glyphs.push(LayoutGlyph {
                    segment_idx: seg_idx,
                    global_char_idx: 0,
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
            for (&ch, &cw) in chars[start..end].iter().zip(word_advances) {
                glyphs.push(LayoutGlyph {
                    segment_idx: seg_idx,
                    global_char_idx: 0,
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
