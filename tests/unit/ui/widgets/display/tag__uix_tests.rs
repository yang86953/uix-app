//! Tag UIX 声明壳回归测试。

use super::*;

/// UIX 声明壳必须保持原 Tag 类型、配置和单叶节点形状。
#[test]
fn uix_shell_preserves_single_tag_kernel_leaf() {
    let node = View::build(
        Tag::new("已完成")
            .color(TagColor::Success)
            .closable()
            .checkable(true),
    );
    assert!(node.children.is_empty());
    assert!(node.widget.as_any().is::<Tag>());
    assert_eq!(
        node.widget.snapshot_fields(),
        SnapshotFields::Tag {
            text: "已完成".to_owned(),
            color: TagColor::Success,
            closable: true,
            font_size: DEFAULT_TAG_FONT_SIZE,
            custom_color: None,
            checkable: true,
            checked: false,
            visible: true,
            icon: String::new(),
        }
    );
}
