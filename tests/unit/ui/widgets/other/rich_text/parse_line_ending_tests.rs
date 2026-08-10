// 引入公开解析入口与段模型，验证完整 Markdown 调度结果。
use super::super::{RichTextSegment, RichTextStyle, parse_rich_text};

// 验证三种 Markdown 行结束都生成同一个 NewLine 契约。
#[test]
// 声明普通文本行结束归一化测试。
fn normalizes_plain_text_line_endings_once() {
    // 混合孤立 CR、CRLF、LF 与尾随 CR。
    let segments = parse_rich_text("甲\r乙\r\n丙\n丁\r");
    // 每个源行结束只能产生一个 NewLine，回车不能进入可见文本。
    assert_eq!(
        // 比较完整有序段列表。
        segments,
        // 构造四行文本及其真实源行结束。
        vec![
            // 第一行保持普通文本。
            RichTextSegment::Text {
                // 保存第一行内容。
                content: "甲".into(),
                // 使用默认文本样式。
                style: RichTextStyle::default(),
            },
            // 孤立 CR 生成一个换行段。
            RichTextSegment::NewLine,
            // 第二行保持普通文本。
            RichTextSegment::Text {
                // 保存第二行内容。
                content: "乙".into(),
                // 使用默认文本样式。
                style: RichTextStyle::default(),
            },
            // CRLF 合并生成一个换行段。
            RichTextSegment::NewLine,
            // 第三行保持普通文本。
            RichTextSegment::Text {
                // 保存第三行内容。
                content: "丙".into(),
                // 使用默认文本样式。
                style: RichTextStyle::default(),
            },
            // LF 保持一个换行段。
            RichTextSegment::NewLine,
            // 第四行保持普通文本。
            RichTextSegment::Text {
                // 保存第四行内容。
                content: "丁".into(),
                // 使用默认文本样式。
                style: RichTextStyle::default(),
            },
            // 尾随孤立 CR 仍只生成一个换行段。
            RichTextSegment::NewLine,
        ],
    );
}

// 验证孤立 CR 可参与需要跨行判定的块级语法。
#[test]
// 声明 Setext 与 ATX 跨孤立回车测试。
fn parses_block_syntax_across_isolated_carriage_returns() {
    // 使用孤立 CR 分隔 Setext 正文、下划线和后续 ATX 标题。
    let segments = parse_rich_text("一级\r===\r# 二级");
    // 两种标题都应经过既有块级解析而不是保留回车字形。
    assert_eq!(
        // 比较完整块级输出顺序。
        segments,
        // 构造两个一级标题及中间源换行。
        vec![
            // Setext 标题沿用 Heading1 样式。
            RichTextSegment::Text {
                // 保存 Setext 正文。
                content: "一级".into(),
                // 使用统一一级标题样式。
                style: RichTextStyle {
                    // 标题使用粗体。
                    bold: true,
                    // 标题字号来自 Typography Heading1。
                    font_size: Some(38.0),
                    // 其他样式保持默认。
                    ..Default::default()
                },
            },
            // Setext 标记行的孤立 CR 成为一个块结束换行。
            RichTextSegment::NewLine,
            // 后续 ATX 标题也正常进入块解析器。
            RichTextSegment::Text {
                // 保存去除井号后的标题正文。
                content: "二级".into(),
                // 同为一级 ATX 标题样式。
                style: RichTextStyle {
                    // 标题使用粗体。
                    bold: true,
                    // 标题字号来自 Typography Heading1。
                    font_size: Some(38.0),
                    // 其他样式保持默认。
                    ..Default::default()
                },
            },
        ],
    );
}

// 验证孤立 CR 分隔的围栏继续复用统一 Code 段。
#[test]
// 声明围栏行结束归一化测试。
fn parses_fences_delimited_by_isolated_carriage_returns() {
    // 使用孤立 CR 分隔开围栏、代码正文、闭围栏与后续文本。
    let segments = parse_rich_text("~~~rust\rlet value = 1;\r~~~\r后续");
    // 围栏正文只保留内部 LF，闭围栏后的换行继续进入统一段模型。
    assert_eq!(
        // 比较完整围栏与后续内容输出。
        segments,
        // 构造代码段、块后换行和普通正文。
        vec![
            // 围栏正文复用既有代码段类型。
            RichTextSegment::Code {
                // 源孤立 CR 已规范化为内部 LF。
                content: "let value = 1;\n".into(),
            },
            // 闭围栏后的孤立 CR 保留为一个换行段。
            RichTextSegment::NewLine,
            // 后续内容恢复普通文本解析。
            RichTextSegment::Text {
                // 保存围栏后的正文。
                content: "后续".into(),
                // 使用默认文本样式。
                style: RichTextStyle::default(),
            },
        ],
    );
}
