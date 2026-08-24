//! Tag UIX 视觉声明回归测试。

use super::*;

/// UIX 视觉声明必须保持原 Tag 类型、配置和单叶节点形状。
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
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Tag>()
        .expect("UIX 根必须保留 Tag 内核");
    assert_eq!(kernel.visual.layout.default_font_size, 12.0);
    assert_eq!(kernel.visual.layout.close_width, 20.0);
    assert_eq!(kernel.visual.check_icon, "check");
    assert!(!kernel.font_size_authored);
}

/// 显式字号必须覆盖 UIX 默认值，完整视觉表则由实例共享。
#[test]
fn uix_visual_configuration_is_shared_and_keeps_authored_font_size() {
    let first = View::build(Tag::new("一").font_size(15.0));
    let second = View::build(Tag::new("二"));
    let first = first
        .widget
        .as_any()
        .downcast_ref::<Tag>()
        .expect("第一个 UIX 根必须保留 Tag 内核");
    let second = second
        .widget
        .as_any()
        .downcast_ref::<Tag>()
        .expect("第二个 UIX 根必须保留 Tag 内核");
    assert_eq!(first.font_size, 15.0);
    assert!(first.font_size_authored);
    assert_eq!(second.font_size, 12.0);
    assert!(std::ptr::eq(first.visual, second.visual));
}

/// 固有尺寸测量缓存文本宽度，字号变化时必须精确失效。
#[test]
fn intrinsic_size_reuses_and_invalidates_text_width_cache() {
    let tag = Tag::new("缓存宽度");
    assert!(tag.cached_text_width.get().is_nan());
    let original = tag.intrinsic_size();
    let cached = tag.cached_text_width.get();
    assert!(cached.is_finite());
    assert_eq!(tag.intrinsic_size(), original);

    let resized = tag.font_size(18.0);
    assert!(resized.cached_text_width.get().is_nan());
    let resized_size = resized.intrinsic_size();
    assert!(resized.cached_text_width.get().is_finite());
    assert!(resized_size.w > original.w);
    assert!(resized_size.h > original.h);
}
