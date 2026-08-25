//! 保存 RichText 的估算字符宽度与全局字符索引映射。

// 单段逻辑文本直接借用公开段内容，多段才拥有拼接结果。
use std::borrow::Cow;

// 引入布局字形、布局行与富文本段类型。
use super::presentation::RichTextMetricsVisual;
use super::{LayoutGlyph, LayoutGlyphKind, LayoutLine, LayoutLineKind, RichTextSegment};

// 返回单个富文本段参与选择、复制与跨段断行的逻辑正文。
pub(super) fn segment_source_text(segment: &RichTextSegment) -> Option<&str> {
    match segment {
        // 文本、代码与链接直接借用其原始正文。
        RichTextSegment::Text { content, .. }
        | RichTextSegment::Code { content }
        | RichTextSegment::Link { content, .. } => Some(content),
        // 图片编解码能力开启时，图片以 alt 参与逻辑文本。
        #[cfg(feature = "image-codecs")]
        RichTextSegment::Image { alt, .. } => Some(alt),
        // 主题分隔线零宽，逻辑换行由紧随其后的 NewLine 唯一拥有。
        RichTextSegment::ThematicBreak => None,
        // 显式换行段借用静态 LF，避免单换行内容申请堆内存。
        RichTextSegment::NewLine => Some("\n"),
    }
}

// 将富文本段投影为保留显式换行的完整逻辑源文本。
pub(super) fn source_text(segments: &[RichTextSegment]) -> Cow<'_, str> {
    // 保存唯一非空贡献段，常见单段文本可直接借用。
    let mut single = "";
    // 统计真正改变逻辑文本的非空段数量。
    let mut contributor_count = 0usize;
    // 同时累计多段拼接的精确 UTF-8 容量。
    let mut source_bytes = 0usize;
    // 一次扫描决定借用或拥有策略。
    for text in segments.iter().filter_map(segment_source_text) {
        source_bytes += text.len();
        if !text.is_empty() {
            contributor_count += 1;
            if contributor_count == 1 {
                single = text;
            }
        }
    }
    // 空文本与单贡献段都不需要建立临时 String。
    if contributor_count <= 1 {
        return Cow::Borrowed(single);
    }
    // 多段只申请一次精确容量，避免逐段 push 触发扩容。
    let mut source = String::with_capacity(source_bytes);
    // 按声明顺序拼接全部逻辑正文。
    for text in segments.iter().filter_map(segment_source_text) {
        source.push_str(text);
    }
    // 返回跨样式段共享的连续逻辑源文本。
    Cow::Owned(source)
}

// 将当前布局字形刷新为一个稳定视觉行。
pub(super) fn flush_line(
    // 接收已完成视觉行列表。
    lines: &mut Vec<LayoutLine>,
    // 接收当前行尚未结算的字形。
    glyphs: &mut Vec<LayoutGlyph>,
    // 接收当前行实际高度。
    line_height: f32,
    // 接收 UIX 声明的统一行高比例。
    metrics: RichTextMetricsVisual,
) {
    // 非空行以全部实际字形的最大字号计算行高，避免后续小字号段压缩前序大字号段。
    let line_height = glyphs
        // 遍历当前行全部待结算字形。
        .iter()
        // 将每个字形字号转换为统一的一点五倍行高。
        .map(|glyph| match glyph.kind {
            // 普通文本继续使用固定一点五倍行高。
            LayoutGlyphKind::Text => glyph.font_size * metrics.line_height_factor,
            // 图片的 font_size 字段保存替换对象实际高度。
            LayoutGlyphKind::InlineImage => glyph.font_size,
        })
        // 选择当前视觉行需要的最大行高。
        .reduce(f32::max)
        // 实际字形行高不得低于调用方提供的稳定默认行高。
        .map(|actual_height| actual_height.max(line_height))
        // 空行没有字形，继续使用调用方提供的默认行高。
        .unwrap_or(line_height);
    // 新行顶部紧随前一行底部，首行从零开始。
    let y = lines.last().map(|line| line.y + line.height).unwrap_or(0.0);
    // 转移当前行字形所有权并清空复用缓冲区。
    let glyphs = std::mem::take(glyphs);
    // 登记完整视觉行。
    lines.push(LayoutLine {
        // 保存计算后的行顶部。
        y,
        // 行高至少保持一个有限正像素。
        height: line_height.max(1.0),
        // 普通结算入口只创建文本行。
        kind: LayoutLineKind::Text,
        // 保存当前行全部字形。
        glyphs,
        // 结束视觉行构造。
    });
}

// 估算字符宽度，单位为逻辑像素。
pub(super) fn char_width(fs: f32, ch: char, metrics: RichTextMetricsVisual) -> f32 {
    // 按字符类别选择与字号相乘的稳定比例。
    match ch {
        // 普通空格使用较窄 advance。
        ' ' => fs * metrics.space_advance,
        // 制表符保留较宽估算 advance。
        '\t' => fs * metrics.tab_advance,
        // 常见宽 Latin 字母使用较宽比例。
        'm' | 'M' | 'W' | 'w' => fs * metrics.wide_advance,
        // 常见窄字母、数字与标点使用较窄比例。
        'i' | 'I' | 'l' | '1' | '.' | ',' | ':' | ';' | '\'' => fs * metrics.narrow_advance,
        // CJK 字符按全字号估算。
        c if is_cjk(c) => fs * metrics.cjk_advance,
        // CJK 标点块按接近全字号估算。
        c if ('\u{3000}'..='\u{303f}').contains(&c) => fs * metrics.cjk_punctuation_advance,
        // 其他字符使用通用半宽估算。
        _ => fs * metrics.default_advance,
        // 结束字符宽度匹配。
    }
}

// 判断字符是否属于常用 CJK、假名、韩文或 Ethiopic 宽字符区间。
fn is_cjk(ch: char) -> bool {
    // 使用既有布局宽字符范围保持估算兼容。
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
