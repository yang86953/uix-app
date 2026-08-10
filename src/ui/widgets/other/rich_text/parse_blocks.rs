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
    // 识别有序列表并保留源文档中的数字与标记前缀。
    if let Some((prefix, content)) = parse_ordered_list(text) {
        // 输出原始数字和列表标记，避免改变作者的编号语义。
        push_text_segment(prefix, &RichTextStyle::default(), segments);
        // 列表正文继续复用普通内联 Markdown 解析。
        parse_inline_range(content, &RichTextStyle::default(), segments);
        // 返回当前行已经由有序列表分支消费。
        return true;
    }
    // 识别引用行并用竖线前缀和斜体表达引用层级。
    if let Some(content) = parse_block_quote(text) {
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

// 解析正文行与下一行组成的 Setext 标题，并返回统一标题样式和正文。
pub(super) fn parse_setext_heading<'a>(
    // 正文生命周期独立于仅用于判定的下划线行。
    text: &'a str,
    // 下划线行只在当前调用中读取。
    underline: &str,
) -> Option<(RichTextStyle, &'a str)> {
    // 空正文或只含空白的行不能形成标题。
    if text.trim().is_empty() {
        // 让下划线行回到原有字面解析路径。
        return None;
    }
    // 已经属于其他受支持块级语法的正文不能被 Setext 重新解释。
    if parse_atx_heading(text).is_some()
        // 无序列表保持列表优先级。
        || parse_unordered_list(text).is_some()
        // 有序列表保持列表优先级。
        || parse_ordered_list(text).is_some()
        // 引用行保持引用优先级。
        || parse_block_quote(text).is_some()
        // 孤立 Setext 标记行本身保持字面语义，不能成为下一标记的正文。
        || parse_setext_underline(text).is_some()
    {
        // 当前正文按原块级语法处理。
        return None;
    }
    // 下划线必须由单一种类的等号或连字符组成。
    let level = parse_setext_underline(underline)?;
    // 返回与 ATX 标题共享的 Typography 样式。
    Some((heading_style(level), text))
}

// 解析 Setext 下划线并返回对应标题级别。
fn parse_setext_underline(text: &str) -> Option<u8> {
    // 只移除块级标记允许的 ASCII 空格和制表符外围空白。
    let marker = text.trim_matches(|ch| ch == ' ' || ch == '\t');
    // 标记至少包含一个可见字符。
    let first = marker.as_bytes().first().copied()?;
    // 一级使用等号，二级使用连字符，其他起始字符无效。
    let level = match first {
        // 等号下划线映射 Heading1。
        b'=' => 1,
        // 连字符下划线映射 Heading2。
        b'-' => 2,
        // 其他字符让整行保持原有字面语义。
        _ => return None,
    };
    // 标记内部只能重复同一个 ASCII 字符。
    if !marker.as_bytes().iter().all(|byte| *byte == first) {
        // 混合或带其他字符的下划线不是 Setext 标记。
        return None;
    }
    // 返回已经验证的标题级别。
    Some(level)
}

// 解析行首一级引用，要求大于号后紧跟空格或制表符。
fn parse_block_quote(text: &str) -> Option<&str> {
    // 先移除引用的大于号标记。
    let remaining = text.strip_prefix('>')?;
    // 读取标记后的第一个字符作为语法分隔符。
    let separator = remaining.chars().next()?;
    // 只接受 Markdown 块级标记使用的 ASCII 空格或制表符。
    if !matches!(separator, ' ' | '\t') {
        // 没有合法分隔符时让原文继续走普通文本解析。
        return None;
    }
    // 跳过一个分隔字符并返回可继续解析内联语法的正文。
    Some(&remaining[separator.len_utf8()..])
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
    // 只去除 Markdown ATX 闭合标记允许的尾随 ASCII 空格与制表符。
    let trimmed = content.trim_end_matches(|ch| ch == ' ' || ch == '\t');
    // 从正文末尾向前扫描连续 ASCII 闭合井号。
    let mut hash_start = trimmed.len();
    // 连续井号均是单字节，回退后仍位于 UTF-8 边界。
    while hash_start > 0 && trimmed.as_bytes()[hash_start - 1] == b'#' {
        // 继续向前纳入同一闭合井号序列。
        hash_start -= 1;
    }
    // 只有闭合井号序列前存在 ASCII 空格或制表符时才把它视为闭合标记。
    if hash_start < trimmed.len()
        && trimmed[..hash_start]
            .chars()
            .next_back()
            .is_some_and(|ch| ch == ' ' || ch == '\t')
    {
        // 去除连续闭合井号以及它前面的 ASCII 分隔空白。
        return trimmed[..hash_start].trim_end_matches(|ch| ch == ' ' || ch == '\t');
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
    // 只接受 Markdown 块级标记使用的 ASCII 空格或制表符。
    if !matches!(separator, ' ' | '\t') {
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
    // 有序列表至少需要一个数字，且块标记编号不得超过 Markdown 规定的九位。
    if digit_len == 0
        // 十位及以上数字序列保留为普通文本，避免把非列表编号误识别为块标记。
        || digit_len > 9
        // 标准 Markdown 的两种有序列表标记都在这里统一识别。
        || !text
            // 读取数字序列后的单字节标记。
            .as_bytes()
            // 点号和右括号以外的字符不启动有序列表。
            .get(digit_len)
            // 只接受两种公开标记。
            .is_some_and(|marker| matches!(marker, b'.' | b')'))
    {
        // 不满足列表标记边界时保留原始文本。
        return None;
    }
    // 读取列表标记后的第一个 Unicode 字符。
    let separator = text[digit_len + 1..].chars().next()?;
    // 列表标记后只接受 Markdown 块级标记使用的 ASCII 空格或制表符。
    if !matches!(separator, ' ' | '\t') {
        // 例如 1.test 应保持普通文本。
        return None;
    }
    // 计算列表正文起点，保留数字、原始标记和一个分隔字符作为前缀。
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
