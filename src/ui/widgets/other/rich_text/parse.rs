//! 富文本解析与布局辅助。

use super::*;

pub fn layout_rich_text_segments(
    segments: &[RichTextSegment],
    max_width: f32,
    default_font_size: f32,
    default_color: Color,
) -> (f32, usize, f32) {
    let (_, total_h, max_w) =
        layout_rich_text(segments, max_width, default_font_size, default_color);
    let char_count: usize = segments
        .iter()
        .map(|s| match s {
            RichTextSegment::NewLine => 1,
            RichTextSegment::Text { content, .. } => content.chars().count(),
            RichTextSegment::Code { content } => content.chars().count(),
            RichTextSegment::Link { content, .. } => content.chars().count(),
        })
        .sum();
    (total_h, char_count, max_w)
}

/// 将纯文本内容解析为 RichTextSegment 列表（支持 ``` 围栏代码块和内联反引号 ` `）
///
/// 围栏代码块 → RichTextSegment::Code，内联反引号 → RichTextSegment::Code，其余文本 → RichTextSegment::Text。
pub fn parse_rich_text(content: &str) -> Vec<RichTextSegment> {
    let mut segments = Vec::new();
    let mut rest = content;
    while let Some(pos) = rest.find("```") {
        let before = &rest[..pos];
        if !before.is_empty() {
            parse_inline_text(before, &mut segments);
        }
        rest = &rest[pos + 3..];
        if let Some(end) = rest.find("```") {
            // 跳过语言标注行
            let code_start = rest.find('\n').map(|n| n + 1).unwrap_or(0);
            let code = if code_start < end {
                &rest[code_start..end]
            } else {
                &rest[..end]
            };
            segments.push(RichTextSegment::Code {
                content: code.to_string(),
            });
            rest = &rest[end + 3..];
        } else {
            parse_inline_text(rest, &mut segments);
            rest = "";
        }
    }
    if !rest.is_empty() {
        parse_inline_text(rest, &mut segments);
    }
    segments
}

/// 解析内联反引号 `code` 并将它们转为 RichTextSegment::Code
fn parse_inline_text(text: &str, segments: &mut Vec<RichTextSegment>) {
    let mut remaining = text;
    while let Some(start) = remaining.find('`') {
        let before = &remaining[..start];
        if !before.is_empty() {
            segments.push(RichTextSegment::Text {
                content: before.to_string(),
                style: RichTextStyle::default(),
            });
        }
        remaining = &remaining[start + 1..];
        if let Some(end) = remaining.find('`') {
            let code = &remaining[..end];
            segments.push(RichTextSegment::Code {
                content: code.to_string(),
            });
            remaining = &remaining[end + 1..];
        } else {
            // 不成对的反引号当作普通文本
            segments.push(RichTextSegment::Text {
                content: format!("`{}", remaining),
                style: RichTextStyle::default(),
            });
            remaining = "";
        }
    }
    if !remaining.is_empty() {
        segments.push(RichTextSegment::Text {
            content: remaining.to_string(),
            style: RichTextStyle::default(),
        });
    }
}

