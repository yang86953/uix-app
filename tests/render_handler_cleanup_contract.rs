//! 验证 VirtualScroll render handler 的注册、替换与清理生命周期。

use uix::prelude::{Label, ViewNode, VirtualScroll, WidgetId};
use uix::ui::__private::traits::Widget;
use uix::ui::__private::{
    WidgetTree, build_view_tree_for_test, reconcile_view_tree_for_test, view_tree_children_for_test,
};

// 构造使用固定索引身份的 VirtualScroll，并让行文案暴露当前 renderer 版本。
fn rendered_virtual_scroll(prefix: &'static str) -> impl uix::ui::View {
    VirtualScroll::new()
        .item_count(4)
        .item_height(10.0)
        .overscan(0)
        .size(100.0, 20.0)
        .render(move |index| ViewNode::leaf(Label::new(format!("{prefix}-{index}"))))
}

// 构造与 renderer 版本相同的 VirtualScroll，但不再携带行 renderer。
fn bare_virtual_scroll() -> ViewNode {
    ViewNode::leaf(
        VirtualScroll::new()
            .item_count(4)
            .item_height(10.0)
            .overscan(0)
            .size(100.0, 20.0),
    )
}

// 收集当前物化行的稳定 key 与运行时子节点身份。
fn keyed_children(tree: &WidgetTree, root: WidgetId) -> Vec<(String, WidgetId)> {
    view_tree_children_for_test(tree, root)
        .into_iter()
        .map(|id| {
            let key = tree
                .get(id)
                .and_then(|node| node.key())
                .expect("VirtualScroll 行必须拥有稳定 key")
                .to_owned();
            (key, id)
        })
        .collect()
}

// 读取指定运行时子节点的 Label 文案，确认新版 renderer 已原位 patch。
fn label_text(tree: &WidgetTree, id: WidgetId) -> String {
    tree.get(id)
        .and_then(|node| node.widget().as_any().downcast_ref::<Label>())
        .map(|label| label.text().to_owned())
        .expect("VirtualScroll 子节点必须是 Label")
}

#[test]
fn virtual_scroll_render_handler_replacement_clears_stale_sidecar() {
    // 首次建树必须注册 renderer 并物化当前视口内的多行子树。
    let mut tree = build_view_tree_for_test(rendered_virtual_scroll("old"));
    let root = tree
        .root_id()
        .expect("VirtualScroll renderer 必须建立运行时根");
    let initial = keyed_children(&tree, root);
    assert_eq!(initial.len(), 2, "固定二十像素视口应物化两行");
    for (key, id) in &initial {
        let index = key
            .strip_prefix("virtual-scroll-item:")
            .expect("普通 VirtualScroll 行必须使用索引 key");
        assert_eq!(label_text(&tree, *id), format!("old-{index}"));
    }

    // 同型新版声明必须替换 sidecar，并按稳定 key 原位更新行内容。
    reconcile_view_tree_for_test(&mut tree, rendered_virtual_scroll("new"));
    let updated = keyed_children(&tree, root);
    assert_eq!(updated, initial, "新版 renderer 必须复用稳定行身份");
    for (key, id) in &updated {
        let index = key
            .strip_prefix("virtual-scroll-item:")
            .expect("更新后的 VirtualScroll 行必须保留索引 key");
        assert_eq!(label_text(&tree, *id), format!("new-{index}"));
    }

    // 协调到同型但无 renderer 的声明时，旧动态子树必须立即清空。
    reconcile_view_tree_for_test(&mut tree, bare_virtual_scroll());
    assert!(
        view_tree_children_for_test(&tree, root).is_empty(),
        "移除 renderer 后不得保留旧物化行"
    );

    // 后续布局也不得从已清理的旧 sidecar 重新物化行。
    tree.layout();
    assert!(
        view_tree_children_for_test(&tree, root).is_empty(),
        "后续 layout 不得复活旧 renderer 的物化行"
    );
}
