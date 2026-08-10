// RichText Markdown 标题解析回归测试。
use super::*;

// 验证行首 ATX 标题复用标题字号并保留内联样式。
#[test]
fn parses_atx_headings_with_inline_styles() {
    // 解析一级标题、可选闭合井号和六级标题。
    let segments = parse_rich_text("# **一级** #\n###### *六级*");
    // 标题应保留换行顺序，并把内联样式叠加到标题基础样式。
    assert_eq!(
        segments,
        vec![
            RichTextSegment::Text {
                content: "一级".into(),
                style: RichTextStyle {
                    bold: true,
                    font_size: Some(38.0),
                    ..Default::default()
                },
            },
            RichTextSegment::NewLine,
            RichTextSegment::Text {
                content: "六级".into(),
                style: RichTextStyle {
                    bold: true,
                    italic: true,
                    font_size: Some(16.0),
                    ..Default::default()
                },
            },
        ]
    );
    // 没有空白分隔符的井号行必须保持普通文本。
    let literal = parse_rich_text("#没有空格");
    // 非 ATX 输入不应意外获得标题样式。
    assert_eq!(
        literal,
        vec![RichTextSegment::Text {
            content: "#没有空格".into(),
            style: RichTextStyle::default(),
        }]
    );
}

// 验证多个连续闭合井号会被去除，而无空白分隔的井号仍保留为正文。
#[test]
fn trims_multiple_atx_closing_hashes() {
    // 解析带有三个连续闭合井号的一级标题。
    let closed = parse_rich_text("# 标题 ###");
    // 连续闭合井号不应进入标题正文。
    assert_eq!(
        closed,
        vec![RichTextSegment::Text {
            content: "标题".into(),
            style: RichTextStyle {
                bold: true,
                font_size: Some(38.0),
                ..Default::default()
            },
        }]
    );
    // 解析正文与井号之间没有空白的标题文本。
    let literal = parse_rich_text("# 标题###");
    // 没有空白边界时井号应保持为标题正文的一部分。
    assert_eq!(
        literal,
        vec![RichTextSegment::Text {
            content: "标题###".into(),
            style: RichTextStyle {
                bold: true,
                font_size: Some(38.0),
                ..Default::default()
            },
        }]
    );
}

// 验证 ATX 闭合井号只把 ASCII 空格与制表符视为语法空白。
#[test]
fn atx_closing_hashes_preserve_unicode_whitespace() {
    // 解析以 NBSP 分隔起始井号的普通文本。
    let unicode_opener = parse_rich_text("#\u{a0}标题");
    // Unicode 空白不得启动 ATX 标题语法。
    assert_eq!(
        // 比较伪标题的完整解析结果。
        unicode_opener,
        // 构造保持原文的默认文本段。
        vec![RichTextSegment::Text {
            // 起始井号、NBSP 与正文都必须保留。
            content: "#\u{a0}标题".into(),
            // 未启动标题时使用默认样式。
            style: RichTextStyle::default(),
        }],
    );

    // 解析以制表符分隔和尾随的合法闭合井号。
    let ascii = parse_rich_text("# 标题\t###\t");
    // ASCII 制表符应允许闭合井号被去除。
    assert_eq!(
        // 比较合法闭合标题的完整解析结果。
        ascii,
        // 构造一级标题期望段。
        vec![RichTextSegment::Text {
            // 合法闭合井号不进入标题正文。
            content: "标题".into(),
            // 一级标题沿用既有粗体字号。
            style: RichTextStyle {
                // 标题保持粗体。
                bold: true,
                // 一级标题保持既有字号。
                font_size: Some(38.0),
                // 其余样式保持默认。
                ..Default::default()
            },
        }],
    );

    // 解析 NBSP 分隔以及全角空格尾随的伪闭合井号。
    let unicode = parse_rich_text("# 标题\u{a0}###\n# 标题 ###\u{3000}");
    // Unicode 空白不属于 ATX 闭合语法，必须与井号一起保留为正文。
    assert_eq!(
        // 比较两种 Unicode 空白位置的完整结果。
        unicode,
        // 构造两行一级标题期望段。
        vec![
            // NBSP 位于正文和井号之间时不得触发闭合。
            RichTextSegment::Text {
                // 保留 NBSP 与尾随井号。
                content: "标题\u{a0}###".into(),
                // 一级标题样式不受边界判定影响。
                style: RichTextStyle {
                    // 标题保持粗体。
                    bold: true,
                    // 一级标题保持既有字号。
                    font_size: Some(38.0),
                    // 其余样式保持默认。
                    ..Default::default()
                },
            },
            // 保留源码换行。
            RichTextSegment::NewLine,
            // 全角空格位于井号之后时整段闭合候选保持正文。
            RichTextSegment::Text {
                // 保留井号与尾随全角空格。
                content: "标题 ###\u{3000}".into(),
                // 一级标题样式不受边界判定影响。
                style: RichTextStyle {
                    // 标题保持粗体。
                    bold: true,
                    // 一级标题保持既有字号。
                    font_size: Some(38.0),
                    // 其余样式保持默认。
                    ..Default::default()
                },
            },
        ],
    );
}
