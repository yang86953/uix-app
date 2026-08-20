// Markdown 行内图片候选的私有语法与本地资源边界。

// 复用统一的 Markdown ASCII 标点转义判定。
use super::is_escaped_markdown_char;

// 保存完整图片候选的解析结果。
pub(super) enum ParsedInlineImage {
    // 合法候选携带已解码的替代文本与本地路径。
    Image {
        // 保存图片替代文本。
        alt: String,
        // 保存本地图片资源路径。
        src: String,
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
impl ParsedInlineImage {
    // 返回整个 Markdown 图片候选的字节长度。
    pub(super) fn consumed(&self) -> usize {
        // 两个结果分支都保存相同语义的消费量。
        match self {
            // 合法图片返回其源范围长度。
            Self::Image { consumed, .. }
            // 字面候选返回其源范围长度。
            | Self::Literal { consumed } => *consumed,
        }
    }
}

// 解析一个从感叹号开始的完整 Markdown 行内图片候选。
pub(super) fn parse_inline_image(text: &str) -> Option<ParsedInlineImage> {
    // 图片候选必须以标准感叹号和左方括号开头。
    if !text.starts_with("![") {
        // 其他输入没有 Markdown 图片结构。
        return None;
    }
    // 查找考虑嵌套和转义的替代文本结束位置。
    let alt_end = find_balanced_delimiter(text, 2, '[', ']')?;
    // Markdown 图片要求目标左括号紧随替代文本闭括号。
    if !text[alt_end..].starts_with("](") {
        // 仅替代文本或其他后缀继续保持普通文本。
        return None;
    }
    // 查找考虑嵌套和转义的图片目标结束位置。
    let src_end = find_balanced_delimiter(text, alt_end + 2, '(', ')')?;
    // 完整候选包含目标闭括号。
    let consumed = src_end + 1;
    // 解码替代文本中的 Markdown 标点转义。
    let alt = unescape_markdown_punctuation(&text[2..alt_end]);
    // 清理目标外围空白后解码已登记的标点转义。
    let src = unescape_markdown_punctuation(text[alt_end + 2..src_end].trim());
    // 空替代文本、空路径、控制字符或 URI 不进入本地图片生命周期。
    if alt.trim().is_empty()
        // 空白路径同样没有可加载资源身份。
        || src.trim().is_empty()
        // 控制字符会破坏稳定的文件系统边界。
        || src.chars().any(char::is_control)
        // 首版只接受本地路径，不接受任何 URI 方案。
        || has_uri_scheme(&src)
        // 双斜杠前缀属于无方案 URI，而不是本地文件路径。
        || src.starts_with("//")
    {
        // 结构完整的非法候选整体保持原始 Markdown 文本。
        return Some(ParsedInlineImage::Literal { consumed });
    }
    // 返回可由父解析器投影为图片段的合法候选。
    Some(ParsedInlineImage::Image {
        // 交付已经解码的替代文本。
        alt,
        // 交付已经解码且限定为本地资源的路径。
        src,
        // 交付完整源范围长度。
        consumed,
    })
}

// 判断图片目标是否使用 URI scheme，同时保留 Windows 盘符路径。
fn has_uri_scheme(src: &str) -> bool {
    // Windows 绝对路径的单字母盘符不属于 URI scheme。
    if src.as_bytes().get(1) == Some(&b':')
        // 盘符后必须紧邻本地目录分隔符。
        && matches!(src.as_bytes().get(2), Some(b'\\' | b'/'))
        // 盘符本身必须是 ASCII 字母。
        && src.as_bytes()[0].is_ascii_alphabetic()
    {
        // 明确保留 C:\ 或 C:/ 形式的本地路径。
        return false;
    }
    // 只检查首个冒号之前的候选 scheme。
    let Some(colon) = src.find(':') else {
        // 没有冒号时不存在显式 URI scheme。
        return false;
    };
    // 提取候选 scheme 字符串。
    let scheme = &src[..colon];
    // RFC 风格 scheme 必须非空且以 ASCII 字母开头。
    !scheme.is_empty()
        // 验证第一个字符。
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        // 其余字符只允许字母、数字、加号、连字符和点。
        && scheme
            // 按字节验证 ASCII scheme。
            .bytes()
            // 全部字符都必须属于 scheme 集合。
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

// 查找替代文本或目标中的平衡闭合分隔符。
fn find_balanced_delimiter(text: &str, start: usize, open: char, close: char) -> Option<usize> {
    // 从开分隔符之后开始扫描候选内部字符。
    let mut cursor = start;
    // 记录未闭合的嵌套开分隔符数量。
    let mut depth = 0usize;
    // 逐个读取 Unicode 字符，保持 UTF-8 边界。
    while let Some(character) = text[cursor..].chars().next() {
        // 反斜杠后的一个字符按字面处理，不参与平衡计数。
        if character == '\\' {
            // 跳过当前反斜杠。
            cursor += character.len_utf8();
            // 同时跳过可能存在的被转义字符。
            if let Some(next) = text[cursor..].chars().next() {
                // 被转义字符不会改变分隔符深度。
                cursor += next.len_utf8();
            }
            // 继续扫描剩余候选。
            continue;
        }
        // 嵌套开分隔符增加待闭合深度。
        if character == open {
            // 记录一层新的嵌套结构。
            depth += 1;
        } else if character == close {
            // 顶层闭分隔符结束当前候选部分。
            if depth == 0 {
                // 返回闭分隔符的字节位置。
                return Some(cursor);
            }
            // 关闭最近一层嵌套分隔符。
            depth -= 1;
        }
        // 向下一个字符推进。
        cursor += character.len_utf8();
    }
    // 没有找到平衡的闭分隔符。
    None
}

// 解码替代文本和路径中的 Markdown 反斜杠转义标点。
fn unescape_markdown_punctuation(text: &str) -> String {
    // 预留原始字节长度，避免常见路径重复扩容。
    let mut output = String::with_capacity(text.len());
    // 从字符串开头逐字符扫描。
    let mut cursor = 0usize;
    // 持续处理直到输入末尾。
    while let Some(character) = text[cursor..].chars().next() {
        // 只解码反斜杠后列入 Markdown 标点集合的字符。
        if character == '\\' {
            // 计算反斜杠之后的字符位置。
            let next_cursor = cursor + character.len_utf8();
            // 读取可能被转义的下一个字符。
            if let Some(next) = text[next_cursor..].chars().next() {
                // 命中 Markdown 标点时去掉反斜杠。
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
        output.push(character);
        // 向下一个 Unicode 字符推进。
        cursor += character.len_utf8();
    }
    // 返回去除已识别转义符后的字符串。
    output
}

// 将图片候选解析回归放入独立测试文件。
