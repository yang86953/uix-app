//! 保存 RichText 的估算字符宽度与全局字符索引映射。

// 引入布局字形、布局行与富文本段类型。
use super::{LayoutGlyph, LayoutLine, RichTextSegment};

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
) {
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
        // 保存当前行全部字形。
        glyphs,
        // 结束视觉行构造。
    });
}

// 估算字符宽度，单位为逻辑像素。
pub(super) fn char_width(fs: f32, ch: char) -> f32 {
    // 按字符类别选择与字号相乘的稳定比例。
    match ch {
        // 普通空格使用较窄 advance。
        ' ' => fs * 0.35,
        // 制表符保留较宽估算 advance。
        '\t' => fs * 2.0,
        // 常见宽 Latin 字母使用较宽比例。
        'm' | 'M' | 'W' | 'w' => fs * 0.7,
        // 常见窄字母、数字与标点使用较窄比例。
        'i' | 'I' | 'l' | '1' | '.' | ',' | ':' | ';' | '\'' => fs * 0.3,
        // CJK 字符按全字号估算。
        c if is_cjk(c) => fs,
        // CJK 标点块按接近全字号估算。
        c if ('\u{3000}'..='\u{303f}').contains(&c) => fs * 0.9,
        // 其他字符使用通用半宽估算。
        _ => fs * 0.55,
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
