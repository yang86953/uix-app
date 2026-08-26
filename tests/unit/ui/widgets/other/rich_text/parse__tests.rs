use super::*;

#[test]
fn parses_markdown_links_and_inline_styles() {
    let segments =
        parse_rich_text("普通 **粗体** *斜体* ~~删除~~ ++下划线++ [链接](https://example.test)");

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
