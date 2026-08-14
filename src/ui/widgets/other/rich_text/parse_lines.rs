// 富文本源码行扫描与跨行 Setext 消费辅助。
use std::borrow::Cow;

use super::{RichTextSegment, parse_blocks, parse_inline_line, parse_inline_range};

// 在块级和内联解析前把 CRLF 与孤立 CR 统一为 LF。
pub(super) fn normalize_markdown_line_endings(content: &str) -> Cow<'_, str> {
    // 没有回车的常见路径直接借用输入，避免不必要分配。
    if !content.contains('\r') {
        // LF 与无换行输入已经符合解析器内部契约。
        return Cow::Borrowed(content);
    }
    // 最终字节数不会超过原输入，按原容量一次性分配。
    let mut normalized = String::with_capacity(content.len());
    // 记录尚未复制的原输入起点。
    let mut cursor = 0usize;
    // 逐个查找回车，同时保持 UTF-8 切片边界。
    while let Some(relative) = content[cursor..].find('\r') {
        // 将相对位置转换为完整输入中的绝对位置。
        let carriage_return = cursor + relative;
        // 复制当前回车前的全部原始文本。
        normalized.push_str(&content[cursor..carriage_return]);
        // 每个 CR 或 CRLF 行结束只生成一个内部 LF。
        normalized.push('\n');
        // 先跳过当前单字节回车。
        cursor = carriage_return + 1;
        // CR 后紧邻 LF 时把二者作为一个 Windows 行结束序列消费。
        if content.as_bytes().get(cursor) == Some(&b'\n') {
            // 跳过已经由内部 LF 代表的源 LF。
            cursor += 1;
        }
    }
    // 复制最后一个回车之后的剩余文本。
    normalized.push_str(&content[cursor..]);
    // 返回由解析边界拥有的规范化内容。
    Cow::Owned(normalized)
}

// 解析 Markdown 源码行，并把显式换行转换为 NewLine 段。
pub(super) fn parse_inline_text(text: &str, segments: &mut Vec<RichTextSegment>) {
    // 记录当前待处理行在原始字符串中的起点。
    let mut line_start = 0;
    // 逐行处理普通内联语法与需要下一行判定的 Setext 标题。
    while let Some((line, has_newline, next_start)) = markdown_line_at(text, line_start) {
        // 允许带 ASCII 前导空白的主题分隔线先于缩进代码块完成行级识别。
        if super::super::thematic_break::is_thematic_break_line(line) {
            // 分隔线标记不进入可选择文本。
            segments.push(RichTextSegment::ThematicBreak);
            // 源码行结束仍由唯一 NewLine 段表示。
            if has_newline {
                // 保留分隔线所在源码行的逻辑换行。
                segments.push(RichTextSegment::NewLine);
            }
            // 当前行已完整消费，从下一源码行继续扫描。
            line_start = next_start;
            // 不再把相同输入交给缩进代码或内联解析器。
            continue;
        }
        // 缩进代码块优先消费完整跨行范围，避免代码正文再次进入块级或内联解析。
        if let Some((code, after_block)) = parse_indented_code_block_at(text, line_start) {
            // 复用现有代码段，让布局、绘制和复制继续共享同一契约。
            segments.push(RichTextSegment::Code { content: code });
            // 跳过已经并入代码段的全部源码行。
            line_start = after_block;
            // 当前代码块已经完整处理，继续扫描其后的普通源码行。
            continue;
        }
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

// 从指定源码行起点解析一个 Markdown 缩进代码块。
fn parse_indented_code_block_at(text: &str, start: usize) -> Option<(String, usize)> {
    // 先读取首行，空白行本身不能启动缩进代码块。
    let (first_line, _, _) = markdown_line_at(text, start)?;
    // 首行必须包含至少一个非空白代码字符。
    if is_markdown_blank_line(first_line) {
        // 将独立空白行交回普通行扫描器保留换行语义。
        return None;
    }
    // 首行必须达到四列 Markdown 缩进并能去除一个缩进前缀。
    strip_indented_code_prefix(first_line)?;
    // 创建保存已确认代码正文的字符串。
    let mut code = String::new();
    // 从调用方指定的首行开始逐行扫描。
    let mut cursor = start;
    // 只提交到最后一个非空缩进代码行，避免吞掉块尾空白行。
    let mut committed_end = start;
    // 暂存可能位于两个代码行之间的空白行数量。
    let mut pending_blank_lines = 0usize;
    // 逐行扩展当前代码块，直到遇到非空且缩进不足的正文。
    while let Some((line, has_newline, next_start)) = markdown_line_at(text, cursor) {
        // Markdown 空白行先暂存，只有后续仍是代码行时才并入代码正文。
        if is_markdown_blank_line(line) {
            // EOF 处没有换行的空白行不属于前一个代码块。
            if !has_newline {
                // 停止扩展并保留最后一个已提交代码行边界。
                break;
            }
            // 记录一个可能的块内空行。
            pending_blank_lines += 1;
            // 继续查看空白行之后是否仍有缩进代码。
            cursor = next_start;
            // 当前空白行尚未提交，继续下一轮判定。
            continue;
        }
        // 非空行必须达到四列缩进，否则当前代码块结束。
        let Some(content) = strip_indented_code_prefix(line) else {
            // 普通正文由外层扫描器从最后提交边界重新处理。
            break;
        };
        // 把两个已确认代码行之间暂存的空白行写入正文。
        for _ in 0..pending_blank_lines {
            // 前一个代码行已保留自身换行，因此每个空白行再追加一个 LF。
            code.push('\n');
        }
        // 清除已经提交的块内空白行计数。
        pending_blank_lines = 0;
        // 去除一个四列缩进前缀后保留其余代码正文。
        code.push_str(content);
        // 源码行带换行时在代码正文中统一保存一个 LF。
        if has_newline {
            // 与围栏代码块保持相同的换行归一化结果。
            code.push('\n');
        }
        // 当前非空代码行已经成为可提交块边界。
        committed_end = next_start;
        // 没有后续源码行时结束当前代码块。
        if !has_newline {
            // EOF 代码行无需继续扫描。
            break;
        }
        // 从当前代码行之后继续扩展同一代码块。
        cursor = next_start;
    }
    // 返回归一化代码正文和最后一个已提交源码边界。
    Some((code, committed_end))
}

// 去除达到四列的 Markdown 缩进前缀并返回代码正文。
fn strip_indented_code_prefix(line: &str) -> Option<&str> {
    // 记录按四列制表位展开后的当前缩进列数。
    let mut columns = 0usize;
    // 按 UTF-8 字符边界扫描行首 ASCII 空格和制表符。
    for (offset, ch) in line.char_indices() {
        // 根据 Markdown 列规则推进缩进宽度。
        match ch {
            // ASCII 空格前进一列。
            ' ' => columns += 1,
            // 制表符前进到下一个四列制表位。
            '\t' => columns += 4 - columns % 4,
            // 其他字符在达到四列前出现时不构成缩进代码块。
            _ => return None,
        }
        // 达到四列后只去除已经消费的缩进字符。
        if columns >= 4 {
            // 当前字符结束位置仍是稳定的 UTF-8 切片边界。
            return Some(&line[offset + ch.len_utf8()..]);
        }
    }
    // 行首缩进不足四列时保持原有字面语义。
    None
}

// 判断源码行是否只包含 Markdown 块级允许的 ASCII 空白。
fn is_markdown_blank_line(line: &str) -> bool {
    // 空字符串以及只含空格/制表符的行都属于空白行。
    line.chars().all(|ch| matches!(ch, ' ' | '\t'))
}

// 将缩进代码块专项回归放在独立文件，保持行扫描实现聚焦。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/other/rich_text/parse_indented_code_tests.rs"]
mod indented_code_tests;

// 将行结束专项回归放在独立文件，保持行扫描实现聚焦。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/other/rich_text/parse_line_ending_tests.rs"]
mod line_ending_tests;
