// RichText Markdown 主题分隔线专项回归测试。
use super::*;

// 验证三种可混合标记、ASCII 空白和换行输出契约。
#[test]
fn parses_thematic_break_markers_before_inline_styles() {
    // 第一行覆盖混合标记与间隔，第二行覆盖强调标记歧义。
    let segments = parse_rich_text(" \t- * _ -\t \n***");
    // 两行都必须成为独立段，只有源文本中的行结束生成 NewLine。
    assert_eq!(
        segments,
        vec![
            // 混合标记行形成独立主题分隔线。
            RichTextSegment::ThematicBreak,
            // 第一行源码换行保留一次。
            RichTextSegment::NewLine,
            // 整行星号在内联强调前被块级分隔线消费。
            RichTextSegment::ThematicBreak,
        ]
    );
}

// 验证最小数量、非法字符、非 ASCII 空白和等号边界。
#[test]
fn keeps_non_thematic_break_lines_literal() {
    // 每行分别覆盖明确排除的等号、标记不足、可见字符和 NBSP。
    let segments = parse_rich_text("===\n--\n---x\n-\u{00a0}-\u{00a0}-");
    // 所有行都应沿用普通字面文本与源换行。
    let text = segments
        // 按段恢复可见文本，确保没有误生成主题分隔线。
        .iter()
        // 将支持的公开段映射为测试文本。
        .map(|segment| match segment {
            // 普通文本保留原字符。
            RichTextSegment::Text { content, .. } => content.as_str(),
            // 显式换行恢复为 LF。
            RichTextSegment::NewLine => "\n",
            // 本输入不得生成其他段类型。
            _ => panic!("unexpected non-literal segment: {segment:?}"),
        })
        // 拼接为原始规范化文本。
        .collect::<String>();
    // 字面输出必须与输入完全一致。
    assert_eq!(text, "===\n--\n---x\n-\u{00a0}-\u{00a0}-");
}

// 验证正文后的连续连字符保持 Setext 优先级。
#[test]
fn setext_heading_precedes_thematic_break_for_following_marker() {
    // 第一组是 Setext 二级标题，第二个独立标记行是主题分隔线。
    let segments = parse_rich_text("标题\n---\n___");
    // Setext 标记不产生分隔线，后续孤立标记才产生。
    assert_eq!(
        segments,
        vec![
            // 正文沿用既有 Heading2 样式。
            RichTextSegment::Text {
                // 保存标题正文。
                content: "标题".into(),
                // 使用既有二级标题视觉样式。
                style: RichTextStyle {
                    // 标题保持粗体。
                    bold: true,
                    // Heading2 使用三十像素字号。
                    font_size: Some(30.0),
                    // 其他样式使用默认值。
                    ..Default::default()
                },
            },
            // Setext 标记行的源码换行只保留一次。
            RichTextSegment::NewLine,
            // 孤立下划线标记形成主题分隔线。
            RichTextSegment::ThematicBreak,
        ]
    );
}

// 验证估算布局把分隔线作为单一默认行高并占满约束宽度。
#[test]
fn thematic_break_layout_uses_one_default_line_box() {
    // 解析带尾随源码换行的单一分隔线。
    let segments = parse_rich_text("---\n");
    // 使用十二像素字号和二百像素内容宽度执行公开估算布局。
    let (height, chars, width) = layout_rich_text_segments(&segments, 200.0, 12.0, Color::black());
    // 普通行高契约为字号的一点五倍。
    assert!((height - 18.0).abs() < f32::EPSILON);
    // 分隔线零宽，只有源换行占一个逻辑字符位置。
    assert_eq!(chars, 1);
    // 分隔线布局占满当前可用内容宽度。
    assert!((width - 200.0).abs() < f32::EPSILON);
}
