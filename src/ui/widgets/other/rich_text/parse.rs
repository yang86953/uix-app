//! 富文本解析与布局辅助。

use super::*;

// 复用独立的块级 Markdown 解析辅助，避免主解析器继续膨胀。
#[path = "parse_blocks.rs"]
mod parse_blocks;

// 复用独立的围栏代码边界解析，区分块级围栏和行内反引号。
#[path = "parse_fences.rs"]
mod parse_fences;

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

/// 将 Markdown 内容解析为 RichTextSegment 列表。
///
/// 当前批次覆盖围栏代码、内联代码、链接、强调、删除线、下划线和换行。
pub fn parse_rich_text(content: &str) -> Vec<RichTextSegment> {
    // 创建按文档顺序保存解析结果的段列表。
    let mut segments = Vec::new();
    // 保留尚未处理的输入切片，行首围栏代码块会从这里切出。
    let mut rest = content;
    // 先处理成对的行首围栏代码块，避免代码内部的 Markdown 标记被再次解释。
    while let Some((open, code_start, close)) = parse_fences::find_fenced_block(rest) {
        // 解析围栏开始前的普通 Markdown 内联内容。
        let before = &rest[..open];
        // 只有存在前置内容时才进入内联解析器。
        if !before.is_empty() {
            // 普通内容继续复用统一的内联解析路径。
            parse_inline_text(before, &mut segments);
        }
        // 只把围栏正文作为代码段内容，语言标注已经在边界辅助中跳过。
        segments.push(RichTextSegment::Code {
            content: rest[code_start..close].to_string(),
        });
        // 继续解析围栏结束标记之后的内容。
        rest = &rest[close + 3..];
    }
    // 处理最后一个围栏之后或未闭合围栏中的剩余 Markdown 内容。
    if !rest.is_empty() {
        // 统一交给内联解析器处理链接、样式与换行。
        parse_inline_text(rest, &mut segments);
    }
    // 返回保持源文档顺序的段列表。
    segments
}

/// 解析内联 Markdown，并把显式换行转换为 NewLine 段。
fn parse_inline_text(text: &str, segments: &mut Vec<RichTextSegment>) {
    // 记录当前行在原始字符串中的起点。
    let mut line_start = 0;
    // 逐字符寻找显式换行，保留空行语义。
    for (offset, ch) in text.char_indices() {
        // 只把换行符本身转换为 NewLine，其他字符交给内联解析器。
        if ch == '\n' {
            // 兼容 Windows 文本中的 CRLF，避免把回车渲染出来。
            let line = text[line_start..offset].trim_end_matches('\r');
            // 解析当前行内的 Markdown 标记。
            parse_inline_line(line, segments);
            // 把源文档中的换行加入统一段模型。
            segments.push(RichTextSegment::NewLine);
            // 从换行之后继续解析下一行。
            line_start = offset + ch.len_utf8();
        }
    }
    // 解析最后一行，尾随换行对应的空行无需额外文本段。
    if line_start < text.len() {
        // 兼容没有换行结尾的普通 Markdown 内容。
        parse_inline_line(&text[line_start..], segments);
    }
}

/// 解析不含显式换行的一行 Markdown 内联内容。
fn parse_inline_line(text: &str, segments: &mut Vec<RichTextSegment>) {
    // 先处理行首块级标记，块级正文仍复用同一个内联解析器。
    if parse_blocks::parse_block_line(text, segments) {
        // 块级标记已经消费当前行，不再重复按普通文本解析。
        return;
    }
    // 普通行从默认样式开始递归解析嵌套的内联标记。
    parse_inline_range(text, &RichTextStyle::default(), segments);
}

/// 递归解析一段共享样式的 Markdown 内联内容。
fn parse_inline_range(text: &str, style: &RichTextStyle, segments: &mut Vec<RichTextSegment>) {
    // 记录尚未输出的普通文本起点。
    let mut text_start = 0;
    // 记录当前扫描位置，始终保持在 UTF-8 字符边界。
    let mut cursor = 0;
    // 扫描整段内容中的代码、链接和样式标记。
    while cursor < text.len() {
        // 先处理 Markdown 反斜杠转义，避免被转义的标记进入解析器。
        if text[cursor..].starts_with('\\') {
            // 读取反斜杠后的第一个 Unicode 字符。
            if let Some(next) = text[cursor + 1..].chars().next() {
                // 只解码 Markdown 标点，其他反斜杠保持原文。
                if is_escaped_markdown_char(next) {
                    // 计算被转义字符在 UTF-8 文本中的结束位置。
                    let escaped_end = cursor + 1 + next.len_utf8();
                    // 先输出转义符之前仍保持当前样式的普通文本。
                    push_text_segment(&text[text_start..cursor], style, segments);
                    // 输出去掉反斜杠后的字面标点。
                    push_text_segment(&text[cursor + 1..escaped_end], style, segments);
                    // 跳过反斜杠和被转义字符。
                    cursor = escaped_end;
                    // 下一段普通文本从转义字符之后开始累计。
                    text_start = cursor;
                    // 已经消费一个转义序列，继续扫描后续内容。
                    continue;
                }
            }
        }
        // 只有特殊起始字符才需要分配临时段列表并尝试解析。
        if may_start_inline_element(text, cursor) {
            // 使用临时段列表探测元素，避免元素先于前置文本写入结果。
            let mut element_segments = Vec::new();
            // 识别当前位置的一个完整 Markdown 内联元素。
            if let Some(consumed) = parse_inline_element(text, cursor, style, &mut element_segments)
            {
                // 先输出元素之前仍保持当前样式的普通文本。
                push_text_segment(&text[text_start..cursor], style, segments);
                // 按源文档顺序追加刚刚解析出的内联元素。
                segments.extend(element_segments);
                // 当前元素已经由解析函数写入结果，跳过其源文本。
                cursor += consumed;
                // 下一段普通文本从元素之后开始累计。
                text_start = cursor;
                // 已经消费一个完整元素，继续扫描其后的内容。
                continue;
            }
        }
        // 未识别的标记按原文推进，保证不成对标记保持字面值。
        cursor += literal_advance(text, cursor);
    }
    // 输出扫描结束后剩余的普通文本。
    push_text_segment(&text[text_start..], style, segments);
}

/// 解析当前位置的代码、链接或样式元素并返回消耗的字节数。
fn parse_inline_element(
    text: &str,
    cursor: usize,
    style: &RichTextStyle,
    segments: &mut Vec<RichTextSegment>,
) -> Option<usize> {
    // 取出从当前位置开始的剩余文本。
    let remaining = &text[cursor..];
    // 内联代码优先于其他标记，保证代码内容按字面解释。
    if remaining.starts_with('`') {
        // 统计开头连续反引号的 delimiter 长度。
        let delimiter_len = backtick_run_len(remaining, 0);
        // 只接受相同长度的连续反引号作为闭合 delimiter。
        if let Some((code_end, delimiter_end)) =
            find_code_closing_delimiter(remaining, delimiter_len, delimiter_len)
        {
            // 读取开闭 delimiter 之间的代码正文。
            let code = &remaining[delimiter_len..code_end];
            // 生成内联代码段。
            segments.push(RichTextSegment::Code {
                content: code.to_string(),
            });
            // 返回开闭 delimiter 和正文的总字节数。
            return Some(delimiter_end);
        }
    }
    // Markdown 链接优先于样式标记，避免链接标签被拆成普通文本。
    if remaining.starts_with('[') {
        // 查找考虑嵌套和转义的链接标签结束位置。
        if let Some(label_end) = find_balanced_link_delimiter(remaining, 1, '[', ']') {
            // 只接受紧随其后的目标起始括号。
            if remaining[label_end..].starts_with("](") {
                // 查找考虑嵌套和转义的链接目标结束位置。
                if let Some(url_end) =
                    find_balanced_link_delimiter(remaining, label_end + 2, '(', ')')
                {
                    // 读取并解码链接显示文本。
                    let label = unescape_markdown_punctuation(&remaining[1..label_end]);
                    // 读取并清理链接 URL 的外围空白。
                    let url =
                        unescape_markdown_punctuation(remaining[label_end + 2..url_end].trim());
                    // 空标签或空 URL 不应被误判为可交互链接。
                    if !label.is_empty() && !url.is_empty() {
                        // 生成保持现有交互契约的 Link 段。
                        segments.push(RichTextSegment::Link {
                            content: label,
                            url,
                        });
                        // 返回整个 Markdown 链接的字节数。
                        return Some(url_end + 1);
                    }
                }
            }
        }
    }
    // 依次尝试双字符和单字符的 Markdown 样式标记。
    for marker in ["**", "__", "~~", "++", "*", "_"] {
        // 只在当前位置确实出现目标标记时继续。
        if !remaining.starts_with(marker) {
            // 当前标记不匹配时继续检查下一个候选标记。
            continue;
        }
        // 单字符标记不能从未闭合双字符标记 run 的前缀启动。
        if marker.len() == 1 && remaining[marker.len()..].starts_with(marker) {
            // 保留双字符标记整体字面值，避免退化成另一种样式。
            continue;
        }
        // 单字符下划线在单词内部不作为强调标记。
        if !can_open_marker(text, cursor, marker) {
            // 不满足边界条件时保留原文字面值。
            continue;
        }
        // 查找同一标记的配对结束位置。
        let Some(close) = find_closing_marker(remaining, marker.len(), marker) else {
            // 未闭合的双字符标记由 literal_advance 一次性跳过。
            continue;
        };
        // 取出标记之间的内容，不允许空样式段。
        let inner = &remaining[marker.len()..close];
        // 合并当前继承样式与 Markdown 标记样式。
        let merged_style = style.merge(&markdown_style(marker));
        // 递归解析样式内部可能出现的链接或其他样式。
        parse_inline_range(inner, &merged_style, segments);
        // 返回开标记、正文和闭标记的总字节数。
        return Some(close + marker.len());
    }
    // 当前字符不是可解析的完整元素。
    None
}

/// 判断当前位置是否可能开始一个 Markdown 内联元素。
fn may_start_inline_element(text: &str, cursor: usize) -> bool {
    // 读取当前位置的第一个 Unicode 字符。
    let Some(ch) = text[cursor..].chars().next() else {
        // 游标到达字符串末尾时没有可解析元素。
        return false;
    };
    // 只有这些 ASCII 标点可能触发内联解析。
    matches!(ch, '`' | '[' | '*' | '_' | '~' | '+')
}

/// 判断反斜杠后的字符是否属于可转义 Markdown 标点。
fn is_escaped_markdown_char(ch: char) -> bool {
    // 覆盖常见行内标记、链接标点和块级标记的字面转义。
    matches!(
        ch,
        '\\' | '`' | '*' | '_' | '[' | ']' | '(' | ')' | '~' | '+' | '#' | '>'
    )
}

/// 返回 Markdown 标记对应的 RichTextStyle 增量。
fn markdown_style(marker: &str) -> RichTextStyle {
    // 创建只包含当前标记效果的样式增量。
    let mut style = RichTextStyle::default();
    // 将 Markdown 标记映射到公开样式字段。
    match marker {
        "**" | "__" => style.bold = true,
        "*" | "_" => style.italic = true,
        "~~" => style.strikethrough = true,
        "++" => style.underline = true,
        _ => {}
    }
    // 返回样式增量。
    style
}

/// 判断当前位置是否可以开启指定 Markdown 标记。
fn can_open_marker(text: &str, cursor: usize, marker: &str) -> bool {
    // 标记后必须存在非空且非空白的正文起点。
    let after = &text[cursor + marker.len()..];
    // 空正文或空白开头都不是有效强调。
    if after.chars().next().is_none_or(char::is_whitespace) {
        // 让空标记保持普通文本。
        return false;
    }
    // 单下划线和双下划线在字母数字单词内部都属于普通字符。
    if marker.starts_with('_') {
        // 读取标记前后的相邻字符。
        let before_char = text[..cursor].chars().next_back();
        // 读取标记后的第一个字符。
        let after_char = after.chars().next();
        // 同时连接单词两侧时不启动 Markdown 强调。
        if before_char.is_some_and(is_word_char) && after_char.is_some_and(is_word_char) {
            // 保留常见 snake_case 文本不被拆分。
            return false;
        }
    }
    // 当前位置满足标记边界要求。
    true
}

/// 判断指定结束位置是否适合作为 Markdown 标记的闭合位置。
fn can_close_marker(text: &str, close: usize, marker: &str) -> bool {
    // 读取样式闭合标记前的相邻字符。
    let before = text[..close].chars().next_back();
    // 闭合标记前不能紧邻空白，否则应按普通文本保留标记。
    if before.is_none_or(char::is_whitespace) {
        // 让搜索器继续寻找后续满足边界的闭合位置。
        return false;
    }
    // 星号、删除线和下划线扩展标记均接受第一个有效闭合位置。
    if !marker.starts_with('_') {
        // 双字符星号和扩展标记不需要额外的单词边界判断。
        return true;
    }
    // 读取下划线后的相邻字符。
    let after = &text[close + marker.len()..];
    // 单词中间的单双下划线都不能闭合强调。
    !(before.is_some_and(is_word_char) && after.chars().next().is_some_and(is_word_char))
}

/// 查找从指定字节位置开始的有效闭合标记。
fn find_closing_marker(text: &str, start: usize, marker: &str) -> Option<usize> {
    // 从正文结束位置开始寻找候选闭合标记。
    let mut search_from = start;
    // 允许跳过不满足边界的候选位置继续寻找。
    while let Some(relative) = text[search_from..].find(marker) {
        // 计算候选闭合标记的绝对位置。
        let candidate = search_from + relative;
        // 三字符强调序列的末尾两个字符应优先闭合外层双字符样式。
        let close = closing_marker_start(text, candidate, marker);
        // 被反斜杠转义的标记不能结束当前样式段。
        if is_escaped_at(text, candidate) {
            // 跳过当前转义标记，继续寻找后续的真实闭合位置。
            search_from = candidate + marker.len();
            // 当前候选已确认不是闭合位置。
            continue;
        }
        // 空正文不构成有效样式段。
        if close > start && can_close_marker(text, close, marker) {
            // 返回第一个满足规则的闭合位置。
            return Some(close);
        }
        // 跳过当前候选，继续搜索后续位置。
        // 从原始候选之后继续寻找，避免调整位置造成重复扫描。
        search_from = candidate + marker.len();
    }
    // 没有找到有效的闭合标记。
    None
}

/// 判断指定字节位置的标点是否被奇数个反斜杠转义。
fn is_escaped_at(text: &str, index: usize) -> bool {
    // 从标点前方向后统计连续反斜杠数量。
    let mut slash_count = 0usize;
    // 只检查紧邻标点的反斜杠，不影响更早的普通文本。
    for byte in text.as_bytes()[..index].iter().rev() {
        // 连续反斜杠构成转义判断依据。
        if *byte == b'\\' {
            // 累加一个紧邻的反斜杠。
            slash_count += 1;
        } else {
            // 遇到其他字符后结束反向扫描。
            break;
        }
    }
    // 奇数个反斜杠转义标点，偶数个反斜杠则恢复标记语义。
    slash_count % 2 == 1
}

/// 查找链接标签或目标的平衡闭合分隔符。
fn find_balanced_link_delimiter(
    text: &str,
    start: usize,
    open: char,
    close: char,
) -> Option<usize> {
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

/// 解码链接标签和目标中的 Markdown 反斜杠转义标点。
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

/// 返回指定位置连续反引号的字节长度。
fn backtick_run_len(text: &str, start: usize) -> usize {
    // 从指定位置开始逐字节统计反引号数量。
    let mut end = start;
    // 连续反引号只占用单字节 ASCII 编码。
    while text.as_bytes().get(end) == Some(&b'`') {
        // 向连续 delimiter 末尾推进一个字节。
        end += 1;
    }
    // 返回连续反引号的长度而不是绝对位置。
    end - start
}

/// 查找与开头长度完全一致的内联代码闭合 delimiter。
fn find_code_closing_delimiter(
    text: &str,
    search_from: usize,
    delimiter_len: usize,
) -> Option<(usize, usize)> {
    // 从开 delimiter 之后开始扫描反引号候选位置。
    let mut cursor = search_from;
    // 防御非法长度，避免空 delimiter 造成无进展循环。
    if delimiter_len == 0 {
        // 空 delimiter 不是有效的 Markdown 代码标记。
        return None;
    }
    // 逐个扫描后续反引号连续序列。
    while let Some(relative) = text[cursor..].find('`') {
        // 计算当前连续反引号序列的绝对起点。
        let candidate = cursor + relative;
        // 读取当前连续反引号序列的长度。
        let run_len = backtick_run_len(text, candidate);
        // 只有长度完全一致的序列才能闭合当前代码段。
        if run_len == delimiter_len {
            // 返回正文结束位置和闭合 delimiter 结束位置。
            return Some((candidate, candidate + run_len));
        }
        // 跳过整段不匹配的反引号，避免把长 delimiter 拆成短 delimiter。
        cursor = candidate + run_len;
    }
    // 没有找到匹配长度的闭合 delimiter。
    None
}

/// 调整连续同类标记中的闭合起点，支持常见嵌套强调写法。
fn closing_marker_start(text: &str, candidate: usize, marker: &str) -> usize {
    // 只有双字符星号或下划线标记需要处理三字符嵌套序列。
    if marker.len() != 2 || !matches!(marker, "**" | "__") {
        // 其他标记沿用第一个候选位置。
        return candidate;
    }
    // 读取当前标记的 ASCII 字节。
    let marker_byte = marker.as_bytes()[0];
    // 从候选标记末尾开始寻找连续同类标记的结束位置。
    let mut run_end = candidate + marker.len();
    // 连续标记仍处于 UTF-8 单字节 ASCII 范围内。
    while text.as_bytes().get(run_end) == Some(&marker_byte) {
        // 把连续标记末尾向后扩展一个字节。
        run_end += 1;
    }
    // 三个及以上标记时取末尾两个作为外层闭合。
    if run_end - candidate > marker.len() {
        // 返回连续标记末尾的双字符起点。
        run_end - marker.len()
    } else {
        // 普通双字符序列直接使用候选起点。
        candidate
    }
}

/// 在无法识别元素时计算安全的 UTF-8 推进长度。
fn literal_advance(text: &str, cursor: usize) -> usize {
    // 未闭合内联代码 delimiter 整体保留，避免连续反引号被拆成多个尝试。
    if text[cursor..].starts_with('`') {
        // 返回当前连续反引号 run 的字节长度。
        return backtick_run_len(text, cursor);
    }
    // 双字符标记未闭合时整体保留，避免第二个字符被误判为单字符标记。
    for marker in ["**", "__", "~~", "++"] {
        // 当前切片以未闭合双字符标记开始时一次跳过它。
        if text[cursor..].starts_with(marker) {
            // 返回双字符标记的字节长度。
            return marker.len();
        }
    }
    // 普通字符按其 UTF-8 字节长度推进。
    text[cursor..].chars().next().map_or(1, char::len_utf8)
}

/// 判断字符是否属于 Markdown 单词边界字符。
fn is_word_char(ch: char) -> bool {
    // 字母数字和下划线共同构成普通标识符字符。
    ch.is_alphanumeric() || ch == '_'
}

/// 追加普通文本段，并合并相邻且样式相同的段。
fn push_text_segment(content: &str, style: &RichTextStyle, segments: &mut Vec<RichTextSegment>) {
    // 空文本不产生无意义的段节点。
    if content.is_empty() {
        // 直接返回并保持段列表紧凑。
        return;
    }
    // 相邻同样式 Text 段可以安全合并，减少布局节点数量。
    if let Some(RichTextSegment::Text {
        content: existing,
        style: existing_style,
    }) = segments.last_mut()
    {
        // 仅在公开样式完全相同的情况下合并内容。
        if existing_style == style {
            // 追加 UTF-8 文本并保留原有段身份。
            existing.push_str(content);
            // 合并完成后无需创建新段。
            return;
        }
    }
    // 样式不同或前一段不是文本时创建新的 Text 段。
    segments.push(RichTextSegment::Text {
        content: content.to_string(),
        style: style.clone(),
    });
}

// 将标题专项测试拆出，保持解析实现文件低于 900 行。
#[cfg(test)]
#[path = "parse_heading_tests.rs"]
mod heading_tests;

// 将块级语法专项测试拆出，保持解析实现文件结构清晰。
#[cfg(test)]
#[path = "parse_block_tests.rs"]
mod block_tests;

// 将围栏代码专项测试拆出，保持解析实现文件结构清晰。
#[cfg(test)]
#[path = "parse_fence_tests.rs"]
mod fence_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_markdown_links_and_inline_styles() {
        let segments = parse_rich_text(
            "普通 **粗体** *斜体* ~~删除~~ ++下划线++ [链接](https://example.test)",
        );

        assert_eq!(
            segments,
            vec![
                RichTextSegment::Text {
                    content: "普通 ".into(),
                    style: RichTextStyle::default(),
                },
                RichTextSegment::Text {
                    content: "粗体".into(),
                    style: RichTextStyle {
                        bold: true,
                        ..Default::default()
                    },
                },
                RichTextSegment::Text {
                    content: " ".into(),
                    style: RichTextStyle::default(),
                },
                RichTextSegment::Text {
                    content: "斜体".into(),
                    style: RichTextStyle {
                        italic: true,
                        ..Default::default()
                    },
                },
                RichTextSegment::Text {
                    content: " ".into(),
                    style: RichTextStyle::default(),
                },
                RichTextSegment::Text {
                    content: "删除".into(),
                    style: RichTextStyle {
                        strikethrough: true,
                        ..Default::default()
                    },
                },
                RichTextSegment::Text {
                    content: " ".into(),
                    style: RichTextStyle::default(),
                },
                RichTextSegment::Text {
                    content: "下划线".into(),
                    style: RichTextStyle {
                        underline: true,
                        ..Default::default()
                    },
                },
                RichTextSegment::Text {
                    content: " ".into(),
                    style: RichTextStyle::default(),
                },
                RichTextSegment::Link {
                    content: "链接".into(),
                    url: "https://example.test".into(),
                },
            ]
        );
    }

    #[test]
    fn preserves_newlines_and_code_literals() {
        let segments = parse_rich_text("上\n`**代码**`\n下");

        assert_eq!(
            segments,
            vec![
                RichTextSegment::Text {
                    content: "上".into(),
                    style: RichTextStyle::default(),
                },
                RichTextSegment::NewLine,
                RichTextSegment::Code {
                    content: "**代码**".into(),
                },
                RichTextSegment::NewLine,
                RichTextSegment::Text {
                    content: "下".into(),
                    style: RichTextStyle::default(),
                },
            ]
        );
    }

    // 验证围栏代码内部的换行进入共享 measure/render 布局路径。
    #[test]
    fn multiline_fenced_code_uses_shared_line_breaks() {
        // 解析包含语言标注和两行正文的围栏代码块。
        let segments = parse_rich_text("```rust\nfn main() {}\nprintln!();\n```");
        // 只保留代码正文的段模型应包含内部换行。
        assert_eq!(
            segments,
            vec![RichTextSegment::Code {
                content: "fn main() {}\nprintln!();\n".into(),
            }]
        );
        // 使用估算布局确认代码换行产生两行而不是换行字形。
        let (_, height, _) = layout_rich_text(&segments, 320.0, 14.0, Color::black());
        // 默认字号的两行行高应为 14 × 1.5 × 2。
        assert_eq!(height, 42.0);
    }

    // 验证反斜杠转义与双层强调标记保持源文本顺序和继承样式。
    #[test]
    fn parses_escaped_punctuation_and_nested_styles() {
        // 解析被转义的星号以及粗体包裹的斜体文本。
        let segments = parse_rich_text(r"\*字面\* **粗体 *斜体***");
        // 外层粗体样式应覆盖普通文字，内层斜体应叠加两种样式。
        assert_eq!(
            segments,
            vec![
                RichTextSegment::Text {
                    content: "*字面* ".into(),
                    style: RichTextStyle::default(),
                },
                RichTextSegment::Text {
                    content: "粗体 ".into(),
                    style: RichTextStyle {
                        bold: true,
                        ..Default::default()
                    },
                },
                RichTextSegment::Text {
                    content: "斜体".into(),
                    style: RichTextStyle {
                        bold: true,
                        italic: true,
                        ..Default::default()
                    },
                },
            ]
        );
        // 链接和代码标点的转义结果应保留为普通文本。
        let escaped = parse_rich_text(r"\[链接\] \`代码\`");
        // 相邻同样式文本会合并为一个普通 Text 段。
        assert_eq!(
            escaped,
            vec![RichTextSegment::Text {
                content: "[链接] `代码`".into(),
                style: RichTextStyle::default(),
            }]
        );
    }

    #[test]
    fn unmatched_markers_and_word_underscores_remain_plain_text() {
        let segments = parse_rich_text("未闭合 **粗体、路径 foo_bar_baz");

        assert_eq!(
            segments,
            vec![RichTextSegment::Text {
                content: "未闭合 **粗体、路径 foo_bar_baz".into(),
                style: RichTextStyle::default(),
            }]
        );
    }

    // 验证合法双下划线仍能加粗，而单词内部双下划线保持字面值。
    #[test]
    fn double_underscore_respects_word_boundaries() {
        // 解析位于独立边界的双下划线粗体。
        let styled = parse_rich_text("__粗体__");
        // 独立双下划线应生成粗体 Text 段。
        assert_eq!(
            styled,
            vec![RichTextSegment::Text {
                content: "粗体".into(),
                style: RichTextStyle {
                    bold: true,
                    ..Default::default()
                },
            }]
        );
        // 解析两侧都连接单词字符的双下划线。
        let literal = parse_rich_text("foo__bar__baz");
        // 单词内部双下划线不应拆分成粗体段。
        assert_eq!(
            literal,
            vec![RichTextSegment::Text {
                content: "foo__bar__baz".into(),
                style: RichTextStyle::default(),
            }]
        );
    }

    // 验证所有样式闭合标记前的空白不会形成有效样式段。
    #[test]
    fn whitespace_before_style_closers_remains_plain_text() {
        // 覆盖粗体、斜体、删除线和扩展下划线标记。
        let inputs = [
            "**粗体 **",
            "__粗体 __",
            "*斜体 *",
            "_斜体 _",
            "~~删除 ~~",
            "++下划 ++",
        ];
        // 逐个确认尾随空白的闭合标记和正文均按字面保留。
        for input in inputs {
            // 解析包含闭合侧尾随空白的样式文本。
            let segments = parse_rich_text(input);
            // 未满足闭合边界时应只保留一个默认样式 Text 段。
            assert_eq!(
                segments,
                vec![RichTextSegment::Text {
                    content: input.into(),
                    style: RichTextStyle::default(),
                }],
                "input: {input}"
            );
        }
    }

    // 验证转义的闭合标记不会提前截断外层斜体段。
    #[test]
    fn escaped_style_closers_do_not_end_the_outer_style() {
        // 解析正文中的字面星号和末尾真实斜体闭合标记。
        let segments = parse_rich_text(r"*包含 \* 字面*");
        // 被转义星号应保留在同一个斜体 Text 段内。
        assert_eq!(
            segments,
            vec![RichTextSegment::Text {
                content: "包含 * 字面".into(),
                style: RichTextStyle {
                    italic: true,
                    ..Default::default()
                },
            }]
        );
    }

    // 验证连续反引号必须使用相同长度闭合，较短反引号保持代码字面量。
    #[test]
    fn code_spans_match_delimiter_length() {
        // 解析包含单个反引号的双反引号代码 span。
        let segments = parse_rich_text("``包含 ` 字面``");
        // 双反引号应包裹完整正文，内部单反引号不能提前闭合。
        assert_eq!(
            segments,
            vec![RichTextSegment::Code {
                content: "包含 ` 字面".into(),
            }]
        );
        // 未找到同长度闭合符时，整个开 delimiter 应保持普通文本。
        let unmatched = parse_rich_text("``未闭合`文字");
        // 未闭合的连续反引号不能误触发单反引号代码解析。
        assert_eq!(
            unmatched,
            vec![RichTextSegment::Text {
                content: "``未闭合`文字".into(),
                style: RichTextStyle::default(),
            }]
        );
    }

    // 验证链接标签和目标中的嵌套、转义分隔符不会提前截断链接。
    #[test]
    fn links_match_balanced_and_escaped_delimiters() {
        // 解析包含转义方括号和转义圆括号的链接。
        let escaped = parse_rich_text(r"[a\]b](https://example.test/a\(b\))");
        // 显示文本与 URL 都应去除已识别的反斜杠转义。
        assert_eq!(
            escaped,
            vec![RichTextSegment::Link {
                content: "a]b".into(),
                url: "https://example.test/a(b)".into(),
            }]
        );
        // 解析 URL 中含有平衡圆括号的链接。
        let nested = parse_rich_text("[页面](https://example.test/a_(b))");
        // 平衡圆括号应属于 URL，最后一个圆括号才结束链接。
        assert_eq!(
            nested,
            vec![RichTextSegment::Link {
                content: "页面".into(),
                url: "https://example.test/a_(b)".into(),
            }]
        );
    }
}
