// 富文本行首块级 Markdown 解析辅助。
use super::{parse_inline_range, push_text_segment, RichTextSegment, RichTextStyle};

// 解析当前行的一级块级标记，并返回是否已经消费整行。
pub(super) fn parse_block_line(text: &str, segments: &mut Vec<RichTextSegment>) -> bool {
    // ATX 标题优先于其他行首标记。
    if let Some((level, content)) = parse_atx_heading(text) {
        // 以项目 Typography 的标题字号建立基础样式。
        let style = heading_style(level);
        // 在标题基础样式上继续解析链接和嵌套强调。
        parse_inline_range(content, &style, segments);
        // 返回当前行已经由标题分支消费。
        return true;
    }
    // 识别无序列表并统一绘制项目符号。
    if let Some(content) = parse_unordered_list(text) {
        // 先输出可见的统一项目符号前缀。
        push_text_segment("• ", &RichTextStyle::default(), segments);
        // 列表正文继续复用普通内联 Markdown 解析。
        parse_inline_range(content, &RichTextStyle::default(), segments);
        // 返回当前行已经由无序列表分支消费。
        return true;
    }
    // 识别有序列表并保留源文档中的数字前缀。
    if let Some((prefix, content)) = parse_ordered_list(text) {
        // 输出原始数字和点号，避免改变作者的编号语义。
        push_text_segment(prefix, &RichTextStyle::default(), segments);
        // 列表正文继续复用普通内联 Markdown 解析。
        parse_inline_range(content, &RichTextStyle::default(), segments);
        // 返回当前行已经由有序列表分支消费。
        return true;
    }
    // 识别引用行并用竖线前缀和斜体表达引用层级。
    if let Some(content) = text.strip_prefix("> ") {
        // 输出稳定的引用视觉前缀。
        push_text_segment("│ ", &RichTextStyle::default(), segments);
        // 引用正文默认使用斜体，同时保留正文内联样式叠加。
        let style = RichTextStyle {
            italic: true,
            ..Default::default()
        };
        // 解析引用正文中的链接和其他 Markdown 内联标记。
        parse_inline_range(content, &style, segments);
        // 返回当前行已经由引用分支消费。
        return true;
    }
    // 当前行没有已支持的块级标记。
    false
}

// 解析行首 ATX 标题并返回标题级别和去除标记后的正文。
fn parse_atx_heading(text: &str) -> Option<(u8, &str)> {
    // 统计行首连续井号数量。
    let mut marker_len = 0usize;
    // 只按 ASCII 字节读取标题标记，避免切断 UTF-8 正文。
    while text.as_bytes().get(marker_len) == Some(&b'#') {
        // 向下一个连续井号推进。
        marker_len += 1;
    }
    // 标题必须包含一到六个井号和至少一个空白分隔符。
    if !(1..=6).contains(&marker_len)
        || !text[marker_len..]
            .chars()
            .next()
            .is_some_and(|ch| ch == ' ' || ch == '\t')
    {
        // 不满足 ATX 边界时保留原始文本。
        return None;
    }
    // 去除标题标记后的前置空白。
    let content = text[marker_len..].trim_start_matches(|ch| ch == ' ' || ch == '\t');
    // 去除可选的行尾闭合井号及其外围空白。
    let content = trim_atx_heading_closer(content);
    // 返回标题级别与可继续解析内联语法的正文。
    Some((marker_len as u8, content))
}

// 去除 ATX 标题末尾可选的闭合井号。
fn trim_atx_heading_closer(content: &str) -> &str {
    // 先去除标题正文末尾的空白，便于判断闭合标记。
    let trimmed = content.trim_end();
    // 只有末尾井号前存在空白时才把它视为闭合标记。
    if trimmed.ends_with('#')
        && trimmed[..trimmed.len() - 1]
            .chars()
            .next_back()
            .is_some_and(|ch| ch == ' ' || ch == '\t')
    {
        // 去除闭合井号以及它前面的分隔空白。
        return trimmed[..trimmed.len() - 1].trim_end();
    }
    // 普通正文中的末尾井号保持字面值。
    trimmed
}

// 解析行首一级无序列表，要求标记后紧跟空格或制表符。
fn parse_unordered_list(text: &str) -> Option<&str> {
    // 读取行首项目符号。
    let marker = text.as_bytes().first().copied()?;
    // 仅支持 Markdown 常见的三种无序列表标记。
    if !matches!(marker, b'-' | b'*' | b'+') {
        // 其他字符不能启动无序列表。
        return None;
    }
    // 读取项目符号后的第一个 Unicode 字符。
    let separator = text[1..].chars().next()?;
    // 没有空白分隔时保留星号等内联标记的原始语义。
    if !separator.is_whitespace() {
        // 例如 *不是列表* 应继续解析为斜体。
        return None;
    }
    // 跳过 ASCII 标记和一个分隔字符，保留后续正文。
    let content_start = 1 + separator.len_utf8();
    // 返回无序列表正文。
    Some(&text[content_start..])
}

// 解析行首一级有序列表，并返回要显示的数字前缀和正文。
fn parse_ordered_list(text: &str) -> Option<(&str, &str)> {
    // 统计行首连续 ASCII 数字数量。
    let mut digit_len = 0usize;
    // 只读取数字字节，确保后续切片仍处在 UTF-8 边界。
    while text
        .as_bytes()
        .get(digit_len)
        .is_some_and(u8::is_ascii_digit)
    {
        // 向下一个连续数字推进。
        digit_len += 1;
    }
    // 有序列表至少需要一个数字和一个点号。
    if digit_len == 0 || text.as_bytes().get(digit_len) != Some(&b'.') {
        // 不满足列表标记边界时保留原始文本。
        return None;
    }
    // 读取点号后的第一个 Unicode 字符。
    let separator = text[digit_len + 1..].chars().next()?;
    // 点号后没有空白时不视为有序列表。
    if !separator.is_whitespace() {
        // 例如 1.test 应保持普通文本。
        return None;
    }
    // 计算列表正文起点，保留数字、点号和一个分隔字符作为前缀。
    let content_start = digit_len + 1 + separator.len_utf8();
    // 返回可见数字前缀和内联正文。
    Some((&text[..content_start], &text[content_start..]))
}

// 返回与 Typography 标题级别一致的基础富文本样式。
fn heading_style(level: u8) -> RichTextStyle {
    // 将六级 Markdown 标题降级到现有五级 Typography 视觉契约。
    let font_size = match level.min(5) {
        // 一级标题沿用 Typography Heading1 的字号。
        1 => 38.0,
        // 二级标题沿用 Typography Heading2 的字号。
        2 => 30.0,
        // 三级标题沿用 Typography Heading3 的字号。
        3 => 24.0,
        // 四级标题沿用 Typography Heading4 的字号。
        4 => 20.0,
        // 五、六级标题沿用 Typography Heading5 的字号。
        _ => 16.0,
    };
    // 标题默认使用粗体并覆盖当前默认字号。
    RichTextStyle {
        // 标题默认使用粗体。
        bold: true,
        // 标题覆盖默认字号。
        font_size: Some(font_size),
        // 其他样式继续使用默认值。
        ..Default::default()
    }
}
