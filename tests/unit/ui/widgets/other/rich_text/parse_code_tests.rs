//! 富文本内联代码解析专项测试。

// 引入父模块的私有解析入口与段模型。
use super::*;

// 验证 Markdown code span 只规范化成对外围空格。
#[test]
fn code_spans_normalize_balanced_outer_spaces() {
    // 解析首尾各有一个空格的内联代码。
    let trimmed = parse_rich_text("` code `");
    // 成对外围空格应各移除一个。
    assert_eq!(
        trimmed,
        vec![RichTextSegment::Code {
            content: "code".into(),
        }]
    );

    // 解析尾部有两个空格的内联代码。
    let partially_trimmed = parse_rich_text("` code  `");
    // 规范化只能移除一对空格，额外尾随空格必须保留。
    assert_eq!(
        partially_trimmed,
        vec![RichTextSegment::Code {
            content: "code ".into(),
        }]
    );

    // 解析正文全部由空格组成的内联代码。
    let all_spaces = parse_rich_text("`  `");
    // 纯空格正文不应被外围空格规则清空。
    assert_eq!(
        all_spaces,
        vec![RichTextSegment::Code {
            content: "  ".into(),
        }]
    );
}
