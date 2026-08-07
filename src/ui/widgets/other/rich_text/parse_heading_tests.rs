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
