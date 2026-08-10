// RichText Markdown 同字符样式嵌套回归测试。

// 引入父模块的私有解析入口与段模型。
use super::*;

// 构造只启用斜体的预期样式。
fn italic_style() -> RichTextStyle {
    // 返回继承默认字段的斜体样式。
    RichTextStyle {
        // 启用外层斜体效果。
        italic: true,
        // 其余样式字段保持默认值。
        ..Default::default()
    }
}

// 构造同时启用粗体与斜体的预期样式。
fn bold_italic_style() -> RichTextStyle {
    // 返回继承默认字段的粗斜体样式。
    RichTextStyle {
        // 启用内层粗体效果。
        bold: true,
        // 保留外层斜体效果。
        italic: true,
        // 其余样式字段保持默认值。
        ..Default::default()
    }
}

// 验证星号斜体可以包裹同字符粗体并共享末尾 delimiter run。
#[test]
// 声明星号反向嵌套测试。
fn parses_italic_around_bold_with_asterisks() {
    // 解析外层单星号和内层双星号共享三个闭合星号的输入。
    let segments = parse_rich_text("*斜体 **粗体***");
    // 外层正文应为斜体，内层正文应叠加粗体。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造按源文档顺序排列的两个文本段。
        vec![
            // 外层斜体正文保持独立文本段。
            RichTextSegment::Text {
                // 保留内层标记前的正文和空格。
                content: "斜体 ".into(),
                // 应用单星号的斜体样式。
                style: italic_style(),
            },
            // 内层粗体正文继承外层斜体。
            RichTextSegment::Text {
                // 去除内外两层 Markdown 标记。
                content: "粗体".into(),
                // 合并粗体与斜体样式。
                style: bold_italic_style(),
            },
        ]
    );
}

// 验证下划线斜体在合法边界下支持同等反向嵌套。
#[test]
// 声明下划线反向嵌套测试。
fn parses_italic_around_bold_with_underscores() {
    // 解析外层单下划线和内层双下划线共享三个闭合下划线的输入。
    let segments = parse_rich_text("_斜体 __粗体___");
    // 下划线写法应得到与星号写法一致的样式段。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造按源文档顺序排列的两个文本段。
        vec![
            // 外层斜体正文保持独立文本段。
            RichTextSegment::Text {
                // 保留内层标记前的正文和空格。
                content: "斜体 ".into(),
                // 应用单下划线的斜体样式。
                style: italic_style(),
            },
            // 内层粗体正文继承外层斜体。
            RichTextSegment::Text {
                // 去除内外两层 Markdown 标记。
                content: "粗体".into(),
                // 合并粗体与斜体样式。
                style: bold_italic_style(),
            },
        ]
    );
}

// 验证分离的内层闭合标记不会提前结束外层斜体。
#[test]
// 声明星号分离闭合测试。
fn preserves_outer_italic_around_separately_closed_bold() {
    // 解析内层双星号在外层单星号之前独立闭合的输入。
    let segments = parse_rich_text("*前 **粗体** 后*");
    // 外层斜体应跨过完整内层粗体并覆盖后续正文。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造外层、内层和外层尾部三个文本段。
        vec![
            // 保留内层标记前的斜体正文。
            RichTextSegment::Text {
                // 保留前置正文和空格。
                content: "前 ".into(),
                // 应用外层斜体样式。
                style: italic_style(),
            },
            // 内层粗体正文叠加两种样式。
            RichTextSegment::Text {
                // 去除内层双星号标记。
                content: "粗体".into(),
                // 合并粗体与斜体样式。
                style: bold_italic_style(),
            },
            // 保留内层闭合后的外层正文。
            RichTextSegment::Text {
                // 保留尾部空格和正文。
                content: " 后".into(),
                // 继续应用外层斜体样式。
                style: italic_style(),
            },
        ]
    );
}

// 验证分离的双下划线闭合也不会提前结束外层斜体。
#[test]
// 声明下划线分离闭合测试。
fn preserves_outer_underscore_italic_around_separately_closed_bold() {
    // 解析内层双下划线在外层单下划线之前独立闭合的输入。
    let segments = parse_rich_text("_前 __粗体__ 后_");
    // 下划线写法应保留完整外层斜体范围。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造外层、内层和外层尾部三个文本段。
        vec![
            // 保留内层标记前的斜体正文。
            RichTextSegment::Text {
                // 保留前置正文和空格。
                content: "前 ".into(),
                // 应用外层斜体样式。
                style: italic_style(),
            },
            // 内层粗体正文叠加两种样式。
            RichTextSegment::Text {
                // 去除内层双下划线标记。
                content: "粗体".into(),
                // 合并粗体与斜体样式。
                style: bold_italic_style(),
            },
            // 保留内层闭合后的外层正文。
            RichTextSegment::Text {
                // 保留尾部空格和正文。
                content: " 后".into(),
                // 继续应用外层斜体样式。
                style: italic_style(),
            },
        ]
    );
}

// 验证未闭合的内层粗体标记只作为外层斜体中的字面文本。
#[test]
// 声明内层未闭合回退测试。
fn keeps_unclosed_inner_bold_literal_inside_outer_italic() {
    // 解析具有合法外层闭合但缺少内层双星号闭合的输入。
    let segments = parse_rich_text("*斜体 **未闭合*");
    // 外层斜体应完整保留，内层双星号应成为字面正文。
    assert_eq!(
        // 传入实际解析结果。
        segments,
        // 构造唯一的外层斜体文本段。
        vec![RichTextSegment::Text {
            // 去除外层标记并保留未闭合的内层双星号。
            content: "斜体 **未闭合".into(),
            // 只应用已经合法闭合的外层斜体。
            style: italic_style(),
        }]
    );
}
