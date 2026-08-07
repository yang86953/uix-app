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
