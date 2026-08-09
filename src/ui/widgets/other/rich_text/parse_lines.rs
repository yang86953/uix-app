// 富文本源码行扫描与跨行 Setext 消费辅助。
use super::{RichTextSegment, parse_blocks, parse_inline_line, parse_inline_range};

// 解析 Markdown 源码行，并把显式换行转换为 NewLine 段。
pub(super) fn parse_inline_text(text: &str, segments: &mut Vec<RichTextSegment>) {
    // 记录当前待处理行在原始字符串中的起点。
    let mut line_start = 0;
    // 逐行处理普通内联语法与需要下一行判定的 Setext 标题。
    while let Some((line, has_newline, next_start)) = markdown_line_at(text, line_start) {
        // 只有存在下一源码行时才可能形成 Setext 标题。
        if has_newline {
            // 读取候选下划线行及其自身的换行边界。
            if let Some((underline, underline_has_newline, after_underline)) =
                // 下一行起点已经由当前行扫描结果确定。
                markdown_line_at(text, next_start)
            {
                // 让块级解析组件判定正文资格、标记种类与标题样式。
                if let Some((style, content)) =
                    // Setext 只消费符合块级边界的正文和下划线行。
                    parse_blocks::parse_setext_heading(line, underline)
                {
                    // 标题正文继续复用统一内联解析和样式合并路径。
                    parse_inline_range(content, &style, segments);
                    // 只有下划线标记行真的以换行结束时才保留块结束换行。
                    if underline_has_newline {
                        // 标记行本身不可见，因此只输出一个块结束 NewLine。
                        segments.push(RichTextSegment::NewLine);
                    }
                    // 一次跳过正文行和已经消费的下划线行。
                    line_start = after_underline;
                    // 当前 Setext 块已经完整处理，继续下一源码行。
                    continue;
                }
            }
        }
        // 非 Setext 行继续使用现有行首块级与内联解析路径。
        parse_inline_line(line, segments);
        // 把当前源码行末尾的显式换行加入统一段模型。
        if has_newline {
            // 空行和普通行都保留一个 NewLine。
            segments.push(RichTextSegment::NewLine);
        }
        // 从当前行结束位置继续扫描下一源码行。
        line_start = next_start;
    }
}

// 返回指定起点的源码行、是否带换行以及下一行起点。
fn markdown_line_at(text: &str, start: usize) -> Option<(&str, bool, usize)> {
    // 起点到达文本末尾时没有额外的虚拟空行。
    if start >= text.len() {
        // 保持既有尾随换行只产生一个 NewLine 的契约。
        return None;
    }
    // 读取从当前行起点开始的剩余文本。
    let remaining = &text[start..];
    // 查找当前源码行的 LF 结束符。
    if let Some(offset) = remaining.find('\n') {
        // CRLF 中的回车不进入可见文本段。
        let line = remaining[..offset].trim_end_matches('\r');
        // LF 是单字节字符，下一行从它之后开始。
        let next_start = start + offset + 1;
        // 返回带显式换行的当前行。
        return Some((line, true, next_start));
    }
    // 没有 LF 时保留最终行的原始内容，包括孤立回车。
    Some((remaining, false, text.len()))
}
