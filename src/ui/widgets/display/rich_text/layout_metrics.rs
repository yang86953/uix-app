//! 保存 RichText 的估算字符宽度与全局字符索引映射。

// 引入布局字形、布局行与富文本段类型。
use super::presentation::RichTextMetricsVisual;
use super::{LayoutGlyph, LayoutGlyphKind, LayoutLine, LayoutLineKind, RichTextSegment};

// 将富文本段拼接为保留显式换行的完整逻辑源文本。
pub(super) fn source_text(segments: &[RichTextSegment]) -> String {
    // 从空字符串开始按声明顺序拼接。
    let mut source = String::new();
    // 遍历全部富文本段。
    for segment in segments {
        // 按段类型追加逻辑源内容。
        match segment {
            // 文本、代码与链接追加其原始正文。
            RichTextSegment::Text { content, .. }
            | RichTextSegment::Code { content }
            | RichTextSegment::Link { content, .. } => source.push_str(content),
            // 图片编解码能力开启时，图片以 alt 参与逻辑选择和复制。
            #[cfg(feature = "image-codecs")]
            // 不把 Markdown 标记或资源路径泄漏到逻辑文本。
            RichTextSegment::Image { alt, .. } => source.push_str(alt),
            // 主题分隔线零宽，逻辑换行由紧随其后的 NewLine 唯一拥有。
            RichTextSegment::ThematicBreak => {}
            // 显式换行段追加一个 LF 作为统一逻辑边界。
            RichTextSegment::NewLine => source.push('\n'),
            // 结束段类型匹配。
        }
    }
    // 返回完整逻辑源文本。
    source
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
