// 引入公开解析入口与段模型，验证完整普通链接投影。
use super::super::{RichTextSegment, RichTextStyle, parse_rich_text};

// 验证解析结果只包含普通文本且串接后精确保留源输入。
fn assert_literal_text(source: &str, segments: &[RichTextSegment]) {
    // 非法链接候选不得生成任何可交互段类型。
    assert!(
        // 检查每个解析段。
        segments
            // 借用有序段列表。
            .iter()
            // 所有段都必须是普通文本。
            .all(|segment| matches!(segment, RichTextSegment::Text { .. }))
    );
    // 按源码顺序串接可能被解析器分批保存的普通文本段。
    let rendered = segments
        // 借用有序段列表。
        .iter()
        // 只读取已经确认的普通文本内容。
        .filter_map(|segment| match segment {
            // 返回普通文本内容。
            RichTextSegment::Text { content, .. } => Some(content.as_str()),
            // 前置断言已拒绝其他分支。
            _ => None,
        })
        // 收集为单一可比较字符串。
        .collect::<String>();
    // 字面解析不得丢失、解码或重排任何源字符。
    assert_eq!(rendered, source);
}

// 验证目标外围空白与百分号编码保持既有合法语义。
#[test]
// 声明合法链接目标字符边界测试。
fn accepts_trimmed_and_percent_encoded_targets() {
    // 解析带外围空白的相对目标和包含百分号编码空格的目标。
    let segments = parse_rich_text("[外围](  docs/guide  ) [编码](a%20b)");
    // 两个目标都不包含实际内部空白或控制字符。
    assert_eq!(
        // 比较完整有序段列表。
        segments,
        // 构造两个链接及中间普通空格。
        vec![
            // 外围空白被清理后的相对链接。
            RichTextSegment::Link {
                // 保留显示标签。
                content: "外围".into(),
                // 只清理目标外围空白。
                url: "docs/guide".into(),
            },
            // 链接之间的空格保持普通文本。
            RichTextSegment::Text {
                // 保存原始分隔空格。
                content: " ".into(),
                // 使用默认文本样式。
                style: RichTextStyle::default(),
            },
            // 百分号编码目标继续进入链接生命周期。
            RichTextSegment::Link {
                // 保留显示标签。
                content: "编码".into(),
                // 百分号编码不是实际空白。
                url: "a%20b".into(),
            },
        ],
    );
}

// 验证内部 Unicode 空白、制表符和控制字符不会生成链接。
#[test]
// 声明非法链接目标字符边界测试。
fn rejects_internal_whitespace_and_control_characters() {
    // 构造内部空格、制表符、NBSP 与 NUL 目标。
    let source = "[空格](a b) [制表](a\tb) [不换行空格](a\u{00a0}b) [控制](a\u{0000}b)";
    // 解析全部结构完整但目标字符非法的候选。
    let segments = parse_rich_text(source);
    // 非法候选必须整体保持普通文本，不得进入链接提交路径。
    assert_literal_text(source, &segments);
}

// 验证空标签和空目标保持完整 Markdown 字面值。
#[test]
// 声明空链接候选拒绝测试。
fn keeps_empty_link_candidates_literal() {
    // 构造空标签与空目标候选。
    let source = "[](docs) [空]()";
    // 解析结构完整但缺少交互必要内容的候选。
    let segments = parse_rich_text(source);
    // 两个候选都不能产生可聚焦 Link 段。
    assert_literal_text(source, &segments);
}
