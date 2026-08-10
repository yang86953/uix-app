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

// 验证引用接受制表符分隔，同时拒绝没有分隔符的相似文本。
#[test]
fn block_quotes_require_space_or_tab_separators() {
    // 解析使用制表符分隔且包含粗体的引用行。
    let quoted = parse_rich_text(">\t**引用**");
    // 引用斜体应与正文粗体叠加，并保留稳定的可见前缀。
    assert_eq!(
        quoted,
        vec![
            RichTextSegment::Text {
                content: "│ ".into(),
                style: RichTextStyle::default(),
            },
            RichTextSegment::Text {
                content: "引用".into(),
                style: RichTextStyle {
                    bold: true,
                    italic: true,
                    ..Default::default()
                },
            },
        ]
    );

    // 解析大于号后没有任何空白分隔符的普通文本。
    let literal = parse_rich_text(">不是引用");
    // 不满足块级边界时应完整保留原始文本。
    assert_eq!(
        literal,
        vec![RichTextSegment::Text {
            content: ">不是引用".into(),
            style: RichTextStyle::default(),
        }]
    );
}

// 验证列表只接受 ASCII 空格或制表符，避免 Unicode 空白误触发块级语义。
#[test]
fn list_markers_require_ascii_space_or_tab_separators() {
    // 解析以制表符分隔的无序列表和有序列表。
    let tabbed = parse_rich_text("-\t项目\n1.\t步骤");
    // 制表符是受支持的 Markdown 列表分隔符。
    assert_eq!(
        tabbed,
        vec![
            RichTextSegment::Text {
                content: "• 项目".into(),
                style: RichTextStyle::default(),
            },
            RichTextSegment::NewLine,
            RichTextSegment::Text {
                content: "1.\t步骤".into(),
                style: RichTextStyle::default(),
            },
        ]
    );

    // 解析使用不换行空格分隔的相似文本。
    let unicode_whitespace = parse_rich_text("-\u{a0}项目\n1.\u{a0}步骤");
    // Unicode 空白不属于列表分隔契约，原文必须完整保留。
    assert_eq!(
        unicode_whitespace,
        vec![
            RichTextSegment::Text {
                content: "-\u{a0}项目".into(),
                style: RichTextStyle::default(),
            },
            RichTextSegment::NewLine,
            RichTextSegment::Text {
                content: "1.\u{a0}步骤".into(),
                style: RichTextStyle::default(),
            },
        ]
    );
}

// 验证有序列表接受点号与右括号标记，并保留作者选择的前缀。
#[test]
fn ordered_lists_accept_dot_and_closing_parenthesis_markers() {
    // 解析点号列表以及正文包含粗体的右括号列表。
    let segments = parse_rich_text("1. 点号项目\n2) **括号项目**");
    // 两种标记都应启动一级有序列表，右括号前缀不得被改写。
    assert_eq!(
        segments,
        vec![
            // 默认样式的点号前缀与正文会合并为同一文本段。
            RichTextSegment::Text {
                content: "1. 点号项目".into(),
                style: RichTextStyle::default(),
            },
            // 源文档中的换行继续保留。
            RichTextSegment::NewLine,
            // 右括号编号前缀保持作者输入。
            RichTextSegment::Text {
                content: "2) ".into(),
                style: RichTextStyle::default(),
            },
            // 列表正文继续复用内联强调解析。
            RichTextSegment::Text {
                content: "括号项目".into(),
                style: RichTextStyle {
                    bold: true,
                    ..Default::default()
                },
            },
        ]
    );

    // 解析缺少 ASCII 分隔符和使用 NBSP 的右括号相似文本。
    let literal = parse_rich_text("3)无空格\n4)\u{a0}非断行空格");
    // 两行都不满足列表边界，必须保持普通文本。
    assert_eq!(
        literal,
        vec![
            // 无分隔符的输入保持完整字面值。
            RichTextSegment::Text {
                content: "3)无空格".into(),
                style: RichTextStyle::default(),
            },
            // 源文档中的换行继续保留。
            RichTextSegment::NewLine,
            // Unicode 空白不属于块级标记分隔符。
            RichTextSegment::Text {
                content: "4)\u{a0}非断行空格".into(),
                style: RichTextStyle::default(),
            },
        ]
    );
}

// 验证有序列表编号只接受一到九位 ASCII 数字。
#[test]
fn ordered_lists_reject_markers_longer_than_nine_digits() {
    // 解析九位点号编号、九位右括号编号以及对应的十位边界。
    let segments = parse_rich_text(
        // 使用换行覆盖两种合法标记与两种超长标记。
        "123456789. **九位点号**\n987654321) 九位括号\n1234567890. 十位点号\n9876543210) 十位括号",
    );
    // 九位标记必须继续进入列表路径，十位标记必须完整保留为普通文本。
    assert_eq!(
        // 声明完整期望段序列以锁定前缀与内联解析边界。
        segments,
        // 构造四行解析结果。
        vec![
            // 九位点号前缀保持作者输入。
            RichTextSegment::Text {
                // 前缀包含分隔空格。
                content: "123456789. ".into(),
                // 前缀使用默认样式。
                style: RichTextStyle::default(),
            },
            // 合法列表正文继续解析粗体。
            RichTextSegment::Text {
                // 去除粗体标记后的正文。
                content: "九位点号".into(),
                // 正文携带粗体样式。
                style: RichTextStyle {
                    // 标记粗体事实。
                    bold: true,
                    // 其余样式保持默认。
                    ..Default::default()
                },
            },
            // 保留源码第一处换行。
            RichTextSegment::NewLine,
            // 九位右括号列表可以与正文合并为默认文本段。
            RichTextSegment::Text {
                // 保留完整合法前缀与正文。
                content: "987654321) 九位括号".into(),
                // 合并段使用默认样式。
                style: RichTextStyle::default(),
            },
            // 保留源码第二处换行。
            RichTextSegment::NewLine,
            // 十位点号编号不得进入列表解析。
            RichTextSegment::Text {
                // 普通文本完整保留输入。
                content: "1234567890. 十位点号".into(),
                // 普通文本使用默认样式。
                style: RichTextStyle::default(),
            },
            // 保留源码第三处换行。
            RichTextSegment::NewLine,
            // 十位右括号编号同样不得进入列表解析。
            RichTextSegment::Text {
                // 普通文本完整保留输入。
                content: "9876543210) 十位括号".into(),
                // 普通文本使用默认样式。
                style: RichTextStyle::default(),
            },
        ],
    );
}
