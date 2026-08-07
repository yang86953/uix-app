// RichText Markdown 列表与引用解析回归测试。
use super::*;

// 验证一级无序列表、有序列表和引用共享内联解析能力。
#[test]
fn parses_first_level_lists_and_block_quotes() {
    // 解析三种行首块级语法及其内部的强调和链接。
    let segments = parse_rich_text("- **项目**\n1. [链接](https://example.test)\n> *引用*");
    // 块级前缀应保持稳定，正文样式应继续由内联解析器生成。
    assert_eq!(
        segments,
        vec![
            RichTextSegment::Text {
                content: "• ".into(),
                style: RichTextStyle::default(),
            },
            RichTextSegment::Text {
                content: "项目".into(),
                style: RichTextStyle {
                    bold: true,
                    ..Default::default()
                },
            },
            RichTextSegment::NewLine,
            RichTextSegment::Text {
                content: "1. ".into(),
                style: RichTextStyle::default(),
            },
            RichTextSegment::Link {
                content: "链接".into(),
                url: "https://example.test".into(),
            },
            RichTextSegment::NewLine,
            RichTextSegment::Text {
                content: "│ ".into(),
                style: RichTextStyle::default(),
            },
            RichTextSegment::Text {
                content: "引用".into(),
                style: RichTextStyle {
                    italic: true,
                    ..Default::default()
                },
            },
        ]
    );
}

// 验证没有空白边界的星号仍保持内联斜体语义而不是列表语义。
#[test]
fn keeps_inline_emphasis_without_list_boundary() {
    // 解析星号紧贴正文的普通 Markdown 行。
    let segments = parse_rich_text("*不是列表*");
    // 没有列表分隔空白时只应生成斜体文本段。
    assert_eq!(
        segments,
        vec![RichTextSegment::Text {
            content: "不是列表".into(),
            style: RichTextStyle {
                italic: true,
                ..Default::default()
            },
        }]
    );
}
