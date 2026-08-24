//! ResultView UIX 视觉声明回归测试。

use super::*;

/// UIX 视觉声明必须保持原 ResultView 类型、配置和单叶节点形状。
#[test]
fn uix_shell_preserves_single_result_kernel_leaf() {
    let node = View::build(
        ResultView::new(ResultType::NotFound)
            .title("页面不存在")
            .extra_text("返回首页"),
    );
    assert!(node.children.is_empty());
    assert!(node.widget.as_any().is::<ResultView>());
    assert_eq!(
        node.widget.snapshot_fields(),
        SnapshotFields::Result {
            result_type: ResultType::NotFound,
            title: "页面不存在".to_owned(),
            subtitle: String::new(),
            extra_text: "返回首页".to_owned(),
        }
    );
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<ResultView>()
        .expect("UIX 根必须保留 ResultView 内核");
    assert_eq!(kernel.intrinsic_size(), Size::new(400.0, 300.0));
    assert_eq!(kernel.visual.typography.title_font_size, 20.0);
    assert_eq!(kernel.visual.status(ResultType::NotFound).icon, "404");
}

/// 每个实例只保存 UIX 配置的静态引用，不复制完整视觉参数表。
#[test]
fn uix_visual_configuration_is_shared_between_result_instances() {
    let first = View::build(ResultView::new(ResultType::Success));
    let second = View::build(ResultView::new(ResultType::Error));
    let first = first
        .widget
        .as_any()
        .downcast_ref::<ResultView>()
        .expect("第一个 UIX 根必须保留 ResultView 内核");
    let second = second
        .widget
        .as_any()
        .downcast_ref::<ResultView>()
        .expect("第二个 UIX 根必须保留 ResultView 内核");
    assert!(std::ptr::eq(first.visual, second.visual));
    assert_eq!(
        std::mem::size_of_val(&first.visual),
        std::mem::size_of::<&'static ResultVisual>()
    );
}

/// 常见单行动作文案必须保持借用，仅真实换行输入才归一化并分配。
#[test]
fn action_text_normalization_keeps_single_line_borrowed() {
    assert!(matches!(
        ResultView::normalized_action_text("返回首页"),
        std::borrow::Cow::Borrowed("返回首页")
    ));
    assert_eq!(
        ResultView::normalized_action_text("返回\n首页"),
        "返回 首页"
    );
}
