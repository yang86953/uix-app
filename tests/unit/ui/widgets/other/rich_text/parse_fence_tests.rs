// RichText Markdown 围栏代码边界回归测试。
use super::*;

// 验证行内三反引号不抢占块级围栏语义，未闭合文本保持完整。
#[test]
fn preserves_unclosed_non_block_fences_as_plain_text() {
    // 解析不在行首且没有闭合的三反引号。
    let segments = parse_rich_text("说明 ```未闭合");
    // 未满足块级围栏边界时，开标记和正文都应保留为普通文本。
    assert_eq!(
        segments,
        vec![RichTextSegment::Text {
            content: "说明 ```未闭合".into(),
            style: RichTextStyle::default(),
        }]
    );
}

// 验证代码正文中的行内三反引号不会提前关闭行首围栏。
#[test]
fn closes_fences_only_at_line_start() {
    // 解析正文含行内三反引号且末尾存在真正闭标记的围栏代码。
    let segments = parse_rich_text("```rust\n代码 ``` 仍在\n正文\n```\n尾部");
    // 行内三反引号应作为代码正文保留，闭标记后的文本继续解析。
    assert_eq!(
        segments,
        vec![
            RichTextSegment::Code {
                content: "代码 ``` 仍在\n正文\n".into(),
            },
            RichTextSegment::NewLine,
            RichTextSegment::Text {
                content: "尾部".into(),
                style: RichTextStyle::default(),
            },
        ]
    );
}

// 验证围栏代码中的 Windows 换行会归一化为统一的 LF。
#[test]
fn normalizes_fenced_code_line_endings() {
    // 使用 CRLF 围栏、正文和闭合行模拟 Windows Markdown 输入。
    let segments = parse_rich_text("```rust\r\nfn main() {}\r\n```\r\n");
    // 代码段应保留正文换行，但不应把回车字符交给布局或复制逻辑。
    assert_eq!(
        segments,
        vec![
            RichTextSegment::Code {
                content: "fn main() {}\n".into(),
            },
            RichTextSegment::NewLine,
        ]
    );
}

// 验证闭合围栏后带非空白后缀时不会提前结束代码块。
#[test]
fn closing_fence_requires_only_trailing_whitespace() {
    // 在代码正文中放置行首三反引号及其非空白后缀。
    let segments = parse_rich_text("```rust\n代码\n```not-close\n正文\n```\n尾部");
    // 带后缀的行应留在 Code 段，最后一个纯围栏才负责闭合。
    assert_eq!(
        segments,
        vec![
            RichTextSegment::Code {
                content: "代码\n```not-close\n正文\n".into(),
            },
            RichTextSegment::NewLine,
            RichTextSegment::Text {
                content: "尾部".into(),
                style: RichTextStyle::default(),
            },
        ]
    );
}

// 验证波浪号围栏复用 Code 段并继续解析闭合后的普通内容。
#[test]
fn parses_tilde_fences_as_code_blocks() {
    // 解析带语言标注的三波浪号围栏。
    let segments = parse_rich_text("~~~rust\n**代码**\n~~~\n尾部");
    // 代码正文不解释强调语法，闭合后的换行和普通文本仍保持顺序。
    assert_eq!(
        segments,
        vec![
            RichTextSegment::Code {
                content: "**代码**\n".into(),
            },
            RichTextSegment::NewLine,
            RichTextSegment::Text {
                content: "尾部".into(),
                style: RichTextStyle::default(),
            },
        ]
    );
}

// 验证闭围栏必须与开围栏同类且长度不少于开围栏。
#[test]
fn requires_matching_marker_and_sufficient_closing_length() {
    // 四反引号开围栏内放置异类波浪号和更短反引号候选。
    let segments = parse_rich_text("````rust\n正文\n~~~\n```\n继续\n`````\n尾部");
    // 异类和更短候选均留在代码正文，五反引号负责闭合。
    assert_eq!(
        segments,
        vec![
            RichTextSegment::Code {
                content: "正文\n~~~\n```\n继续\n".into(),
            },
            RichTextSegment::NewLine,
            RichTextSegment::Text {
                content: "尾部".into(),
                style: RichTextStyle::default(),
            },
        ]
    );
}

// 验证 Unicode 空白不能充当闭围栏后缀，允许的 ASCII 空白不会进入可见段。
#[test]
fn consumes_only_allowed_closing_fence_whitespace() {
    // NBSP 候选后继续放置带空格和制表符的合法闭围栏。
    let segments = parse_rich_text("~~~\n正文\n~~~\u{00a0}\n继续\n~~~ \t\n尾部");
    // NBSP 行保留在代码正文，合法闭围栏尾随空白被完整消费。
    assert_eq!(
        segments,
        vec![
            RichTextSegment::Code {
                content: "正文\n~~~\u{00a0}\n继续\n".into(),
            },
            RichTextSegment::NewLine,
            RichTextSegment::Text {
                content: "尾部".into(),
                style: RichTextStyle::default(),
            },
        ]
    );
}

// 验证未闭合的波浪号围栏保持普通文本，不吞掉后续源码行。
#[test]
fn preserves_unclosed_tilde_fence_as_plain_text() {
    // 构造只有开围栏而没有同类闭围栏的多行输入。
    let segments = parse_rich_text("~~~rust\n后续 **正文**");
    // 未闭合围栏回到普通内联路径，后续强调仍按原有语义解析。
    assert_eq!(
        segments,
        vec![
            RichTextSegment::Text {
                content: "~~~rust".into(),
                style: RichTextStyle::default(),
            },
            RichTextSegment::NewLine,
            RichTextSegment::Text {
                content: "后续 ".into(),
                style: RichTextStyle::default(),
            },
            RichTextSegment::Text {
                content: "正文".into(),
                style: RichTextStyle {
                    bold: true,
                    ..Default::default()
                },
            },
        ]
    );
}
