//! 富文本解析与布局辅助。

use super::*;

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
    // 保留尚未处理的输入切片，围栏代码块会从这里切出。
    let mut rest = content;
    // 先处理围栏代码块，避免代码内部的 Markdown 标记被再次解释。
    while let Some(pos) = rest.find("```") {
        // 解析围栏开始前的普通 Markdown 内联内容。
        let before = &rest[..pos];
        // 只有存在前置内容时才进入内联解析器。
        if !before.is_empty() {
            // 普通内容继续复用统一的内联解析路径。
            parse_inline_text(before, &mut segments);
        }
        // 跳过围栏开始标记。
        rest = &rest[pos + 3..];
        // 查找对应的围栏结束标记。
        if let Some(end) = rest.find("```") {
            // 兼容围栏后的语言标注行。
            let code_start = rest.find('\n').map(|n| n + 1).unwrap_or(0);
            // 只把围栏正文作为代码段内容。
            let code = if code_start < end {
                // 语言标注存在且正文非空时跳过标注。
                &rest[code_start..end]
            } else {
                // 没有可跳过的标注时保留围栏正文。
                &rest[..end]
            };
            // 保持一个代码段，便于复制按钮继续对应整个代码块。
            segments.push(RichTextSegment::Code {
                content: code.to_string(),
            });
            // 继续解析围栏结束后的内容。
            rest = &rest[end + 3..];
        } else {
            // 未闭合围栏按普通内联文本处理，避免吞掉后续内容。
            parse_inline_text(rest, &mut segments);
            // 标记剩余内容已经全部消费。
            rest = "";
        }
    }
    // 处理最后一个围栏之后剩余的 Markdown 内容。
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
    // 从默认样式开始递归解析嵌套的内联标记。
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
        // 使用临时段列表探测元素，避免元素先于前置文本写入结果。
        let mut element_segments = Vec::new();
        // 识别当前位置的一个完整 Markdown 内联元素。
        if let Some(consumed) = parse_inline_element(text, cursor, style, &mut element_segments) {
            // 先输出元素之前仍保持当前样式的普通文本。
            push_text_segment(&text[text_start..cursor], style, segments);
            // 按源文档顺序追加刚刚解析出的内联元素。
            segments.extend(element_segments);
            // 当前元素已经由解析函数写入结果，跳过其源文本。
            cursor += consumed;
            // 下一段普通文本从元素之后开始累计。
            text_start = cursor;
        } else {
            // 未识别的标记按原文推进，保证不成对标记保持字面值。
            cursor += literal_advance(text, cursor);
        }
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
        // 查找配对的结束反引号。
        if let Some(end) = remaining[1..].find('`') {
            // 计算代码正文的结束位置。
            let end = end + 1;
            // 读取反引号之间的代码正文。
            let code = &remaining[1..end];
            // 生成内联代码段。
            segments.push(RichTextSegment::Code {
                content: code.to_string(),
            });
            // 返回两个反引号和正文的总字节数。
            return Some(end + 1);
        }
    }
    // Markdown 链接优先于样式标记，避免链接标签被拆成普通文本。
    if remaining.starts_with('[') {
        // 查找链接标签的右方括号。
        if let Some(label_end) = remaining[1..].find(']') {
            // 右方括号在剩余文本中的绝对位置。
            let label_end = label_end + 1;
            // 只接受紧随其后的目标起始括号。
            if remaining[label_end..].starts_with("](") {
                // 查找链接目标的右括号。
                if let Some(url_end) = remaining[label_end + 2..].find(')') {
                    // 计算链接目标在剩余文本中的结束位置。
                    let url_end = label_end + 2 + url_end;
                    // 读取链接显示文本。
                    let label = &remaining[1..label_end];
                    // 读取并清理链接 URL 的外围空白。
                    let url = remaining[label_end + 2..url_end].trim();
                    // 空标签或空 URL 不应被误判为可交互链接。
                    if !label.is_empty() && !url.is_empty() {
                        // 生成保持现有交互契约的 Link 段。
                        segments.push(RichTextSegment::Link {
                            content: label.to_string(),
                            url: url.to_string(),
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
    // 下划线在字母数字单词内部属于普通字符。
    if marker == "_" {
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
    // 双字符标记按 Markdown 的简单配对规则接受第一个闭合位置。
    if marker != "_" {
        // 双字符和星号标记不需要额外的单词边界判断。
        return true;
    }
    // 读取下划线后的相邻字符。
    let after = &text[close + marker.len()..];
    // 读取下划线前的相邻字符。
    let before = text[..close].chars().next_back();
    // 单词中间的下划线不能闭合强调。
    !(before.is_some_and(is_word_char) && after.chars().next().is_some_and(is_word_char))
}

/// 查找从指定字节位置开始的有效闭合标记。
fn find_closing_marker(text: &str, start: usize, marker: &str) -> Option<usize> {
    // 从正文结束位置开始寻找候选闭合标记。
    let mut search_from = start;
    // 允许跳过不满足边界的候选位置继续寻找。
    while let Some(relative) = text[search_from..].find(marker) {
        // 计算候选闭合标记的绝对位置。
        let close = search_from + relative;
        // 空正文不构成有效样式段。
        if close > start && can_close_marker(text, close, marker) {
            // 返回第一个满足规则的闭合位置。
            return Some(close);
        }
        // 跳过当前候选，继续搜索后续位置。
        search_from = close + marker.len();
    }
    // 没有找到有效的闭合标记。
    None
}

/// 在无法识别元素时计算安全的 UTF-8 推进长度。
fn literal_advance(text: &str, cursor: usize) -> usize {
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
}
