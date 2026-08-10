// RichText Markdown 反斜杠转义边界回归测试。

// 引入父模块的私有解析入口与段模型。
use super::*;

// 验证完整 ASCII 标点集合都能由反斜杠转为字面文本。
#[test]
// 声明 ASCII 标点转义覆盖测试。
fn escapes_every_ascii_punctuation_character() {
    // 显式列出 Markdown 可转义的完整 ASCII 标点集合，避免测试遗漏旧实现未覆盖的符号。
    let punctuation = r##"!"#$%&'()*+,-./:;<=>?@[\]^_`{|}~"##;
    // 为每个标点增加一个反斜杠，构造覆盖完整集合的 Markdown 输入。
    let escaped: String = punctuation
        // 逐个读取显式集合中的标点。
        .chars()
        // 为当前标点生成转义符和字面字符。
        .flat_map(|character| ['\\', character])
        // 收集为一次完整的解析输入。
        .collect();
    // 解析包含全部 ASCII 标点转义的输入。
    let segments = parse_rich_text(&escaped);
    // 所有转义符都应被移除，并合并为一个默认样式文本段。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造唯一的预期普通文本段。
        vec![RichTextSegment::Text {
            // 保留显式标点集合本身。
            content: punctuation.into(),
            // ASCII 标点转义不改变当前文本样式。
            style: RichTextStyle::default(),
        }]
    );
}

// 验证新补齐的标点转义仍继承外层 Markdown 样式。
#[test]
// 声明样式内部标点转义测试。
fn escaped_ascii_punctuation_inherits_outer_style() {
    // 在粗体正文内覆盖旧实现未识别的六种代表性 ASCII 标点。
    let segments = parse_rich_text(r"**\!\-\.\{\|\}**");
    // 转义后的字面标点应保留粗体样式且不泄露反斜杠。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造继承粗体样式的唯一文本段。
        vec![RichTextSegment::Text {
            // 使用去除反斜杠后的标点正文。
            content: "!-.{|}".into(),
            // 只启用外层粗体样式。
            style: RichTextStyle {
                // 继承外层 Markdown 粗体标记。
                bold: true,
                // 其余样式字段保持默认值。
                ..Default::default()
            },
        }]
    );
}

// 验证反斜杠不会吞掉非 ASCII 标点或普通字符。
#[test]
// 声明非 Markdown 标点字面保留测试。
fn preserves_backslashes_before_non_ascii_or_alphanumeric_characters() {
    // 解析非 ASCII 逗号、汉字和数字前的反斜杠。
    let segments = parse_rich_text(r"\，\字\7");
    // 不属于 ASCII 标点的字符必须连同反斜杠保持原文。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造保持全部源字符的普通文本段。
        vec![RichTextSegment::Text {
            // 反斜杠与后续字符均不得被解析器消费。
            content: r"\，\字\7".into(),
            // 未触发 Markdown 样式时保持默认样式。
            style: RichTextStyle::default(),
        }]
    );
}
