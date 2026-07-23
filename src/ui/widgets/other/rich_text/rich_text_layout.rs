//! 富文本布局引擎模块。
//!
//! 提供字符级文本布局、断行、字形缓存等功能。
//! 从 `rich_text.rs` 拆分出来以遵守 900 行文件限制。

use super::RichTextSegment;
use crate::core::Rect;
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::font::text_backend::TextLayoutOptions;
use crate::draw::{Color, FontHandle};

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
    lines: &mut Vec<LayoutLine>,
    glyphs: &mut Vec<LayoutGlyph>,
    current_x: &mut f32,
    max_line_w: &mut f32,
) {
    let shared_url: Option<std::sync::Arc<str>> = link_url.map(std::sync::Arc::from);
    let tokens: Vec<&str> = content.split_inclusive(' ').collect();

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
        let measured = layout.glyphs.get(i).map(|glyph| glyph.width);
        advances.push(
            measured
                .filter(|width| width.is_finite() && *width > 0.0)
                .unwrap_or_else(|| char_width(fs, *ch)),
        );
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
        while end < total && chars[end] != ' ' {
            end += 1;
        }
        if end < total && chars[end] == ' ' {
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
