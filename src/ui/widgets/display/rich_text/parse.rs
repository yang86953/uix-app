//! 富文本解析与布局辅助。
use super::*;
// 复用独立的块级 Markdown 解析辅助，避免主解析器继续膨胀。
#[path = "parse_blocks.rs"]
mod parse_blocks;

// 复用独立的围栏代码边界解析，区分块级围栏和行内反引号。
#[path = "parse_fences.rs"]
mod parse_fences;

// 复用独立的内联代码正文规范化，保持主解析器聚焦语法调度。
#[path = "parse_code_span.rs"]
mod parse_code_span;

// 尖括号自动链接校验独立归入链接解析辅助模块。
#[path = "parse_autolink.rs"]
mod parse_autolink;
// 普通行内链接的分隔符、解码与目标字符校验保持在私有辅助边界。
#[path = "parse_link.rs"]
mod parse_link;
// 把图片语法边界隔离为独立解析组件；能力关闭时仍用它完整保留字面候选。
#[path = "parse_image.rs"]
mod parse_image;
// 跨行扫描与 Setext 消费独立归入行解析辅助模块。
#[path = "parse_lines.rs"]
mod parse_lines;

// 同字符粗斜体嵌套的 delimiter 扫描保持在 RichText 私有解析边界。
#[path = "parse_style.rs"]
mod parse_style;
/// 计算富文本片段在给定宽度下的高度、逻辑字符数与最大行宽。
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
            // 图片能力开启时 alt 是图片替换对象的逻辑文本。
            #[cfg(feature = "image-codecs")]
            RichTextSegment::Image { alt, .. } => alt.chars().count(),
            RichTextSegment::ThematicBreak => 0,
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
/// 当前覆盖围栏/缩进/内联代码、链接、标题、主题分隔线、列表/引用、样式与换行。
pub fn parse_rich_text(content: &str) -> Vec<RichTextSegment> {
    // 创建按文档顺序保存解析结果的段列表。
    let mut segments = Vec::new();
    // 先把 CRLF 与孤立 CR 统一为 LF，让所有语法共享同一分行契约。
    let normalized = parse_lines::normalize_markdown_line_endings(content);
    // 保留尚未处理的规范化输入切片，行首围栏代码块会从这里切出。
    let mut rest = normalized.as_ref();
    // 先处理成对的行首围栏代码块，避免代码内部的 Markdown 标记被再次解释。
    while let Some(block) = parse_fences::find_fenced_block(rest) {
        // 解析围栏开始前的普通 Markdown 内联内容。
        let before = &rest[..block.open];
        // 只有存在前置内容时才进入内联解析器。
        if !before.is_empty() {
            // 普通内容继续复用统一的内联解析路径。
            parse_lines::parse_inline_text(before, &mut segments);
        }
        // 只把围栏正文作为代码段内容，语言标注已经在边界辅助中跳过。
        // 入口已经统一行结束，围栏正文可直接进入共享代码段生命周期。
        segments.push(RichTextSegment::Code {
            content: rest[block.code_start..block.close_start].to_owned(),
        });
        // 继续解析闭围栏行尾之后的内容，允许的尾随空白不会进入可见段。
        rest = &rest[block.after_close..];
    }
    // 处理最后一个围栏之后或未闭合围栏中的剩余 Markdown 内容。
    if !rest.is_empty() {
        // 统一交给内联解析器处理链接、样式与换行。
        parse_lines::parse_inline_text(rest, &mut segments);
    }
    // 返回保持源文档顺序的段列表。
    segments
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
    // 优先识别感叹号图片，避免内部标签在能力关闭时退化为普通链接。
    if remaining.starts_with("![") {
        // 私有图片辅助返回合法图片或完整字面候选。
        if let Some(parsed) = parse_image::parse_inline_image(remaining) {
            // 读取完整候选消费量。
            let consumed = parsed.consumed();
            // 图片能力开启时根据资源边界投影公开图片段或普通文本。
            #[cfg(feature = "image-codecs")]
            match parsed {
                // 合法本地图片使用默认固有尺寸策略。
                parse_image::ParsedInlineImage::Image { alt, src, .. } => {
                    // 输出编译期门控的公开图片段。
                    segments.push(RichTextSegment::Image {
                        // 保存本地资源路径。
                        src,
                        // 保存复制与无障碍替代文本。
                        alt,
                        // Markdown 不覆盖固有宽度。
                        width: None,
                        // Markdown 不覆盖固有高度。
                        height: None,
                        // 默认保持固有比例居中。
                        fit: true,
                        // Markdown 默认没有额外圆角。
                        radius: None,
                    });
                }
                // 非本地或非法候选完整保持字面值。
                parse_image::ParsedInlineImage::Literal { .. } => {
                    // 沿用外层样式保存原始 Markdown 标记。
                    push_text_segment(&remaining[..consumed], style, segments);
                }
            }
            // 图片能力关闭时完整候选只能保持普通文本。
            #[cfg(not(feature = "image-codecs"))]
            {
                // 显式消费合法候选字段，确保关闭能力构建没有潜伏未读状态。
                if let parse_image::ParsedInlineImage::Image { alt, src, .. } = parsed {
                    // 字段只用于分类，不进入任何公开图片段。
                    let _ = (alt, src);
                }
                // 保留完整 Markdown 标记，禁止内部标签退化成 Link。
                push_text_segment(&remaining[..consumed], style, segments);
            }
            // 返回图片候选完整源范围。
            return Some(consumed);
        }
    }
    // 内联代码优先于其他标记，保证代码内容按字面解释。
    if remaining.starts_with('`') {
        // 统计开头连续反引号的 delimiter 长度。
        let delimiter_len = backtick_run_len(remaining, 0);
        // 只接受相同长度的连续反引号作为闭合 delimiter。
        if let Some((code_end, delimiter_end)) =
            find_code_closing_delimiter(remaining, delimiter_len, delimiter_len)
        {
            // 读取开闭 delimiter 之间的代码正文。
            let code = parse_code_span::normalize_code_span(&remaining[delimiter_len..code_end]);
            // 生成内联代码段。
            segments.push(RichTextSegment::Code { content: code });
            // 返回开闭 delimiter 和正文的总字节数。
            return Some(delimiter_end);
        }
    }
    // Markdown 链接优先于样式标记，避免链接标签被拆成普通文本。
    if remaining.starts_with('[') {
        // 私有链接辅助返回合法链接或需要原样保留的完整候选。
        if let Some(parsed) = parse_link::parse_inline_link(remaining) {
            // 读取完整候选已经消费的源字节数。
            let consumed = parsed.consumed();
            // 根据校验结果生成交互链接或继承当前样式的字面文本。
            match parsed {
                // 合法链接继续进入唯一 Link 段和提交生命周期。
                parse_link::ParsedInlineLink::Link { label, url, .. } => {
                    // 生成保持现有交互契约的 Link 段。
                    segments.push(RichTextSegment::Link {
                        // 使用已解码的显示标签。
                        content: label,
                        // 使用已解码并验证字符边界的目标。
                        url,
                    });
                }
                // 结构完整但内容非法的候选必须整体保持字面值。
                parse_link::ParsedInlineLink::Literal { .. } => {
                    // 保留源标记，同时继承外层已有样式。
                    push_text_segment(&remaining[..consumed], style, segments);
                }
            }
            // 返回整个普通链接候选的字节数。
            return Some(consumed);
        }
    }
    // 尖括号 HTTP(S)/邮件自动链接复用现有 Link 段和统一交互路径。
    if remaining.starts_with('<') {
        // 只接受通过 URL 或邮件边界校验的完整目标。
        if let Some((display, url)) = parse_autolink::parse_angle_autolink(remaining) {
            // 邮件目标增加 mailto，HTTP(S) 目标保持原始 URL。
            segments.push(RichTextSegment::Link {
                // 自动链接没有独立标签，直接显示原始候选文本。
                content: display.to_string(),
                // 使用自动链接校验器生成的提交目标。
                url,
            });
            // 开闭尖括号各占一个 ASCII 字节。
            return Some(display.len() + 2);
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
        // 单字符外层需要跳过成对的同字符双标记，其他样式沿用普通闭合搜索。
        let close = if marker.len() == 1 {
            // 让私有样式辅助识别外层斜体中的同字符粗体区间。
            parse_style::find_single_closing_marker(remaining, marker.len(), marker)
        } else {
            // 双字符样式继续使用既有闭合和三字符调整契约。
            find_closing_marker(remaining, marker.len(), marker)
        };
        // 未找到成对闭合时保留源文本。
        let Some(close) = close else {
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
    matches!(ch, '`' | '[' | '<' | '*' | '_' | '~' | '+' | '!')
}

/// 判断反斜杠后的字符是否属于可转义 Markdown 标点。
fn is_escaped_markdown_char(ch: char) -> bool {
    // Markdown 允许反斜杠转义任意 ASCII 标点，统一覆盖块级和行内边界。
    ch.is_ascii_punctuation()
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
