// RichText Markdown 尖括号 HTTP(S) 自动链接回归测试。
use super::*;

// 验证 HTTP 与 HTTPS 自动链接复用现有 Link 段并保留原始目标。
#[test]
fn parses_http_and_https_angle_autolinks() {
    // 解析普通文本之间的小写 HTTPS 与大写 HTTP scheme。
    let segments = parse_rich_text(
        "文档 <https://example.test/guide?q=1> 镜像 <HTTP://example.test/download>",
    );
    // 两个自动链接应保持输入 URL，并由普通文本段分隔。
    assert_eq!(
        segments,
        vec![
            // 自动链接之前的普通文本保持默认样式。
            RichTextSegment::Text {
                content: "文档 ".into(),
                style: RichTextStyle::default(),
            },
            // HTTPS 自动链接直接显示并提交原始 URL。
            RichTextSegment::Link {
                content: "https://example.test/guide?q=1".into(),
                url: "https://example.test/guide?q=1".into(),
            },
            // 两个链接之间的普通文本保持原样。
            RichTextSegment::Text {
                content: " 镜像 ".into(),
                style: RichTextStyle::default(),
            },
            // scheme 匹配忽略 ASCII 大小写，但链接值保留作者拼写。
            RichTextSegment::Link {
                content: "HTTP://example.test/download".into(),
                url: "HTTP://example.test/download".into(),
            },
        ]
    );
}

// 验证歧义、不完整与非 HTTP(S) 候选保持字面文本。
#[test]
fn keeps_invalid_angle_autolinks_literal() {
    // 收集不能进入自动链接交互链的代表性输入。
    let inputs = [
        // 非 HTTP(S) scheme 不属于本批公开契约。
        "<ftp://example.test>",
        // URL 内含空白时保持字面文本。
        "<https://example test>",
        // scheme 后没有目标时不得创建空链接。
        "<https://>",
        // 未闭合尖括号不能吞掉后续文本。
        "<https://example.test",
    ];
    // 逐项验证解析结果只有一个默认样式文本段。
    for input in inputs {
        // 解析当前非法或不完整候选。
        let segments = parse_rich_text(input);
        // 候选必须逐字保留，不能产生 Link 段。
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

// 验证反斜杠转义尖括号只输出字面 URL 文本。
#[test]
fn escaped_angle_autolink_remains_text() {
    // 转义开闭尖括号，阻止候选进入自动链接解析。
    let segments = parse_rich_text(r"\<https://example.test\>");
    // 转义符被解码，但完整内容仍是不可交互的普通文本。
    assert_eq!(
        segments,
        vec![RichTextSegment::Text {
            content: "<https://example.test>".into(),
            style: RichTextStyle::default(),
        }]
    );
}
