// 引入被测无障碍转换与公开角色。
use super::{AccessibilityRole, RichTextSegment, rich_text_accessibility};

// 单一主题分隔线内容必须直接暴露 separator 角色。
#[test]
fn standalone_thematic_break_exposes_separator_role() {
    // 模拟带源码尾随换行的独立分隔线。
    let snapshot = rich_text_accessibility(
        // 分隔线与其行结束属于同一语义块。
        &[RichTextSegment::ThematicBreak, RichTextSegment::NewLine],
        // 分隔线没有聚焦链接。
        None,
    );
    // 快照必须输出标准 separator 角色。
    assert_eq!(snapshot.role, AccessibilityRole::Separator);
    // separator 不应伪造可读名称。
    assert_eq!(snapshot.name, None);
}

// 混合文档保持单一文本节点，同时不朗读 Markdown 标记。
#[test]
fn mixed_rich_text_keeps_text_role_without_break_markers() {
    // 构造文本与主题分隔线混合的单 widget 内容。
    let snapshot = rich_text_accessibility(
        // 直接提供最小混合段列表。
        &[
            // 分隔线前正文。
            RichTextSegment::Text {
                // 保存可朗读正文。
                content: "上".to_owned(),
                // 使用默认文本样式。
                style: Default::default(),
            },
            // 前一源码行结束。
            RichTextSegment::NewLine,
            // 零宽主题分隔线。
            RichTextSegment::ThematicBreak,
            // 分隔线源码行结束。
            RichTextSegment::NewLine,
        ],
        // 混合内容没有聚焦链接。
        None,
    );
    // 当前单节点模型对混合文档保持 Text 角色。
    assert_eq!(snapshot.role, AccessibilityRole::Text);
    // 名称保留两个换行，但不包含源 Markdown 标记。
    assert_eq!(snapshot.name.as_deref(), Some("上\n\n"));
}

// 单一图片 RichText 必须暴露图片语义和 alt 名称。
#[cfg(feature = "image-codecs")]
#[test]
fn standalone_inline_image_exposes_image_role_and_alt() {
    // 构造不依赖真实资源加载的公开图片段。
    let snapshot = rich_text_accessibility(
        // 只提供一个图片原子。
        &[RichTextSegment::Image {
            // 路径只参与资源身份，不进入可访问名称。
            src: "assets/cover.png".into(),
            // alt 作为图片名称。
            alt: "产品封面".into(),
            // 使用固有宽度。
            width: None,
            // 使用固有高度。
            height: None,
            // 默认保持比例。
            fit: true,
            // 不使用圆角。
            radius: None,
        }],
        // 图片不拥有链接焦点。
        None,
    );
    // 快照必须使用标准图片角色。
    assert_eq!(snapshot.role, AccessibilityRole::Image);
    // 可访问名称只包含 alt。
    assert_eq!(snapshot.name.as_deref(), Some("产品封面"));
}
