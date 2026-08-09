// RichText Markdown Setext 标题跨行解析回归测试。
use super::*;

// 验证 Setext 一级和二级标题共享现有标题样式与内联解析。
#[test]
fn parses_setext_headings_with_inline_styles() {
    // 解析带斜体正文的一级标题、二级标题和后续普通正文。
    let segments = parse_rich_text("一级 *强调*\n \t===\t \n二级\n---\n正文");
    // 下划线行不可见，两个标题块各只保留一个结束换行。
    assert_eq!(
        segments,
        vec![
            // 一级标题普通部分沿用 Heading1 粗体字号。
            RichTextSegment::Text {
                content: "一级 ".into(),
                style: RichTextStyle {
                    bold: true,
                    font_size: Some(38.0),
                    ..Default::default()
                },
            },
            // 内联斜体叠加在一级标题基础样式上。
            RichTextSegment::Text {
                content: "强调".into(),
                style: RichTextStyle {
                    bold: true,
                    italic: true,
                    font_size: Some(38.0),
                    ..Default::default()
                },
            },
            // 一级标题标记行的换行成为块结束换行。
            RichTextSegment::NewLine,
            // 二级标题沿用 Heading2 粗体字号。
            RichTextSegment::Text {
                content: "二级".into(),
                style: RichTextStyle {
                    bold: true,
                    font_size: Some(30.0),
                    ..Default::default()
                },
            },
            // 二级标题标记行的换行成为块结束换行。
            RichTextSegment::NewLine,
            // 后续正文继续使用默认样式。
            RichTextSegment::Text {
                content: "正文".into(),
                style: RichTextStyle::default(),
            },
        ]
    );
}

// 验证 Setext 标记行在 EOF 与尾随换行下的输出边界。
#[test]
fn setext_heading_preserves_only_marker_line_ending() {
    // 解析标记行直接结束于 EOF 的标题。
    let eof = parse_rich_text("标题\n===");
    // 标记行没有源换行时不应伪造 NewLine。
    assert_eq!(
        eof,
        vec![RichTextSegment::Text {
            content: "标题".into(),
            style: RichTextStyle {
                bold: true,
                font_size: Some(38.0),
                ..Default::default()
            },
        }]
    );
    // 解析标记行带尾随换行的标题。
    let trailing = parse_rich_text("标题\n===\n");
    // 标记行的源换行应生成且只生成一个块结束 NewLine。
    assert_eq!(
        trailing,
        vec![
            // 标题正文沿用 Heading1 样式。
            RichTextSegment::Text {
                content: "标题".into(),
                style: RichTextStyle {
                    bold: true,
                    font_size: Some(38.0),
                    ..Default::default()
                },
            },
            // 尾随换行只保留一次。
            RichTextSegment::NewLine,
        ]
    );
}

// 验证其他块级正文和非法下划线不会被重新解释为 Setext 标题。
#[test]
fn keeps_ineligible_or_invalid_setext_lines_literal() {
    // ATX 标题保持原优先级，后续连字符行继续作为普通文本。
    let atx = parse_rich_text("# ATX\n---");
    // ATX 与孤立连字符行都应保留各自既有语义。
    assert_eq!(
        atx,
        vec![
            // ATX 标题仍使用 Heading1 样式。
            RichTextSegment::Text {
                content: "ATX".into(),
                style: RichTextStyle {
                    bold: true,
                    font_size: Some(38.0),
                    ..Default::default()
                },
            },
            // ATX 源行的换行保持可见段边界。
            RichTextSegment::NewLine,
            // 孤立连字符行保持普通文本。
            RichTextSegment::Text {
                content: "---".into(),
                style: RichTextStyle::default(),
            },
        ]
    );
    // 混合标记下划线不满足 Setext 契约。
    let mixed = parse_rich_text("正文\n=-=-");
    // 正文与非法标记行都按普通文本和源换行输出。
    assert_eq!(
        mixed,
        vec![
            // 正文保持默认样式。
            RichTextSegment::Text {
                content: "正文".into(),
                style: RichTextStyle::default(),
            },
            // 正文源行的换行继续保留。
            RichTextSegment::NewLine,
            // 混合标记行保持字面文本。
            RichTextSegment::Text {
                content: "=-=-".into(),
                style: RichTextStyle::default(),
            },
        ]
    );
    // 连续两个纯标记行都不具备 Setext 正文资格。
    let stacked = parse_rich_text("---\n===");
    // 两个孤立标记行保持普通文本及其源换行。
    assert_eq!(
        stacked,
        vec![
            // 第一个孤立标记行保持字面文本。
            RichTextSegment::Text {
                content: "---".into(),
                style: RichTextStyle::default(),
            },
            // 两个源码行之间的换行继续保留。
            RichTextSegment::NewLine,
            // 第二个孤立标记行也保持字面文本。
            RichTextSegment::Text {
                content: "===".into(),
                style: RichTextStyle::default(),
            },
        ]
    );
}
