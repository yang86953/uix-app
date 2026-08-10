// 普通 Markdown 行内链接的私有解析与目标字符边界。
use super::is_escaped_markdown_char;

// 保存完整链接候选的解析结果。
pub(super) enum ParsedInlineLink {
    // 合法候选携带已解码的显示标签与提交目标。
    Link {
        // 保存链接显示文本。
        label: String,
        // 保存链接提交目标。
        url: String,
        // 保存整个 Markdown 候选消耗的字节数。
        consumed: usize,
    },
    // 结构完整但内容非法的候选保持源文本字面值。
    Literal {
        // 保存整个 Markdown 候选消耗的字节数。
        consumed: usize,
    },
}

// 暴露两种结果共享的源字节消费量。
impl ParsedInlineLink {
    // 返回整个 Markdown 链接候选的字节长度。
    pub(super) fn consumed(&self) -> usize {
        // 两个结果分支都保存相同语义的消费量。
        match self {
            // 合法链接返回其源范围长度。
            Self::Link { consumed, .. }
            // 字面候选返回其源范围长度。
            | Self::Literal { consumed } => *consumed,
        }
    }
}

// 解析一个从左方括号开始的完整普通行内链接候选。
pub(super) fn parse_inline_link(text: &str) -> Option<ParsedInlineLink> {
    // 调用方只应把左方括号候选交给此辅助边界。
    if !text.starts_with('[') {
        // 其他输入没有普通行内链接结构。
        return None;
    }
    // 查找考虑嵌套和转义的链接标签结束位置。
    let label_end = find_balanced_delimiter(text, 1, '[', ']')?;
    // 普通行内链接要求目标左括号紧随标签闭括号。
    if !text[label_end..].starts_with("](") {
        // 仅标签或其他后缀继续走既有普通内联解析路径。
        return None;
    }
    // 查找考虑嵌套和转义的链接目标结束位置。
    let url_end = find_balanced_delimiter(text, label_end + 2, '(', ')')?;
    // 完整候选包含目标闭括号。
    let consumed = url_end + 1;
    // 解码链接显示文本中的 Markdown 标点转义。
    let label = unescape_markdown_punctuation(&text[1..label_end]);
    // 清理目标外围空白后再解码已登记的标点转义。
    let url = unescape_markdown_punctuation(text[label_end + 2..url_end].trim());
    // 空标签、空目标或含内部空白/控制字符的目标不能进入交互生命周期。
    if label.is_empty() || url.is_empty() || !has_valid_target_characters(&url) {
        // 结构完整的非法候选整体保持原始 Markdown 文本。
        return Some(ParsedInlineLink::Literal { consumed });
    }
    // 返回可由父解析器投影为唯一 Link 段的合法候选。
    Some(ParsedInlineLink::Link {
        // 交付已经解码的显示文本。
        label,
        // 交付已经解码且字符合法的提交目标。
        url,
        // 交付完整源范围长度。
        consumed,
    })
}

// 判断清理并解码后的目标是否可作为稳定提交字符串。
fn has_valid_target_characters(url: &str) -> bool {
    // 实际空白或控制字符必须使用百分号编码等字面安全形式表达。
    !url.chars()
        // 检查每个 Unicode 标量值。
        .any(|ch| ch.is_whitespace() || ch.is_control())
}

// 查找链接标签或目标的平衡闭合分隔符。
fn find_balanced_delimiter(text: &str, start: usize, open: char, close: char) -> Option<usize> {
    // 从开分隔符之后开始扫描链接内部字符。
    let mut cursor = start;
    // 记录未闭合的嵌套开分隔符数量。
    let mut depth = 0usize;
    // 逐个读取 Unicode 字符，确保游标始终位于 UTF-8 边界。
    while let Some(ch) = text[cursor..].chars().next() {
        // 反斜杠后的一个字符按字面处理，不参与平衡计数。
        if ch == '\\' {
            // 跳过当前反斜杠。
            cursor += ch.len_utf8();
            // 同时跳过被转义的下一个字符。
            if let Some(next) = text[cursor..].chars().next() {
                // 被转义字符不会改变链接分隔符嵌套深度。
                cursor += next.len_utf8();
            }
            // 继续扫描剩余链接内容。
            continue;
        }
        // 嵌套开分隔符增加待闭合深度。
        if ch == open {
            // 记录一层新的嵌套链接分隔符。
            depth += 1;
        } else if ch == close {
            // 顶层闭分隔符结束当前链接部分。
            if depth == 0 {
                // 返回闭分隔符的字节位置。
                return Some(cursor);
            }
            // 先关闭最近一层嵌套分隔符。
            depth -= 1;
        }
        // 向下一个字符推进。
        cursor += ch.len_utf8();
    }
    // 没有找到平衡的闭分隔符。
    None
}

// 解码链接标签和目标中的 Markdown 反斜杠转义标点。
fn unescape_markdown_punctuation(text: &str) -> String {
    // 预留不小于原始字节长度的容量，避免常见路径重复扩容。
    let mut output = String::with_capacity(text.len());
    // 从字符串开头逐字符扫描转义序列。
    let mut cursor = 0usize;
    // 持续处理直到输入字符串末尾。
    while let Some(ch) = text[cursor..].chars().next() {
        // 只解码反斜杠后列入 Markdown 标点集合的字符。
        if ch == '\\' {
            // 计算反斜杠之后的字符位置。
            let next_cursor = cursor + ch.len_utf8();
            // 读取可能被转义的下一个字符。
            if let Some(next) = text[next_cursor..].chars().next() {
                // 命中 Markdown 标点时去掉反斜杠并保留字符本身。
                if is_escaped_markdown_char(next) {
                    // 写入解码后的标点。
                    output.push(next);
                    // 一次跳过反斜杠和被转义字符。
                    cursor = next_cursor + next.len_utf8();
                    // 继续处理剩余输入。
                    continue;
                }
            }
        }
        // 普通字符或未命中的反斜杠保持原样。
        output.push(ch);
        // 向下一个 Unicode 字符推进。
        cursor += ch.len_utf8();
    }
    // 返回去除已识别转义符后的字符串。
    output
}

// 将普通链接目标边界回归放在独立文件中。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/other/rich_text/parse_link_tests.rs"]
mod tests;
