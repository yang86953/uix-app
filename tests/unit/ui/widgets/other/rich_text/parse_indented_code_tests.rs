// 引入公开解析入口与受测段模型。
use super::super::{RichTextSegment, RichTextStyle, parse_rich_text};

// 验证四空格、单制表符和混合缩进按四列规则合并为同一代码块。
#[test]
fn parses_four_column_indented_code_prefixes() {
    // 构造三种达到四列的缩进形式、CRLF 换行和块后普通正文。
    let segments = parse_rich_text("    four\r\n\tone tab\r\n \tmixed\r\nplain");
    // 三个缩进行必须合并为一个代码段，普通正文继续单独解析。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造保持源码顺序的预期段列表。
        vec![
            // 代码段去除每行一个四列缩进前缀并保留源换行。
            RichTextSegment::Code {
                // 三行代码统一使用 LF，并保留块末源码换行。
                content: "four\none tab\nmixed\n".to_owned(),
            },
            // 块后的普通正文不得被代码段吞掉。
            RichTextSegment::Text {
                // 保留普通正文内容。
                content: "plain".to_owned(),
                // 普通正文继续使用默认样式。
                style: RichTextStyle::default(),
            },
        ],
    );
}

// 验证块内空白行被保留，而块尾空白行继续由普通换行路径处理。
#[test]
fn preserves_internal_but_not_trailing_blank_lines() {
    // 在两个代码行之间和代码块结尾各放置一个空白源码行。
    let segments = parse_rich_text("    alpha\n\n\tbeta\n\nplain");
    // 只有夹在两个缩进行之间的空白行属于代码正文。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造代码块、普通空行和后续正文的预期顺序。
        vec![
            // 两个代码行合并并保留内部空行。
            RichTextSegment::Code {
                // 块尾源码换行保留，但其后的空白行不进入代码正文。
                content: "alpha\n\nbeta\n".to_owned(),
            },
            // 块尾空白源码行由普通扫描器输出一个换行段。
            RichTextSegment::NewLine,
            // 空白行之后的正文恢复普通文本解析。
            RichTextSegment::Text {
                // 保留后续普通正文。
                content: "plain".to_owned(),
                // 后续正文继续使用默认样式。
                style: RichTextStyle::default(),
            },
        ],
    );
}

// 验证不足四列和 Unicode 空白不会误启动缩进代码块。
#[test]
fn rejects_insufficient_or_unicode_indentation() {
    // 构造三列 ASCII 缩进与一个 NBSP 起始行。
    let segments = parse_rich_text("   three\n\u{00a0}unicode\n");
    // 两行都必须保持普通文本与显式换行语义。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造逐行普通解析的预期结果。
        vec![
            // 三列缩进行保持普通文本。
            RichTextSegment::Text {
                // 不去除不足四列的空格。
                content: "   three".to_owned(),
                // 使用默认文本样式。
                style: RichTextStyle::default(),
            },
            // 保留第一行源码换行。
            RichTextSegment::NewLine,
            // NBSP 不属于 Markdown 缩进列。
            RichTextSegment::Text {
                // Unicode 空白完整保留在正文中。
                content: "\u{00a0}unicode".to_owned(),
                // 使用默认文本样式。
                style: RichTextStyle::default(),
            },
            // 保留第二行源码换行。
            RichTextSegment::NewLine,
        ],
    );
}

// 验证 EOF 缩进代码行不会伪造额外换行。
#[test]
fn indented_code_at_eof_has_no_synthetic_newline() {
    // 构造没有行结束符的最终缩进代码行。
    let segments = parse_rich_text("    tail");
    // EOF 代码正文只能包含去除缩进后的原始字符。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造单一代码段预期结果。
        vec![RichTextSegment::Code {
            // 不为 EOF 伪造换行。
            content: "tail".to_owned(),
        }],
    );
}
