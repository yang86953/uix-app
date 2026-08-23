use crate::ui::adapter::ViewAdapter;
use crate::ui::widgets::{Container, FloatButton};
use crate::ui::{Placement, Space, ViewNode};

// 读取当前队列是否仍含布局工作，避免测试依赖窗口驱动私有辅助函数。
fn has_layout_work(tree: &crate::ui::WidgetTree) -> bool {
    tree.invalidation()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .has_layout()
}

#[test]
fn layout_traversal_is_read_only_and_layout_consumes_snapshot() {
    // 构造没有动态子项的最小真实树，使后续队列变化只来自布局消费。
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(Space::new().width(24.0).height(16.0)));

    assert!(has_layout_work(&tree));
    assert!(!tree.layout_traverse().is_empty());
    // 诊断和规划入口不得偷取消费正式布局工作。
    assert!(has_layout_work(&tree));

    tree.layout();

    // 正式布局已经处理快照中的 Layout；Paint damage 由呈现阶段另行消费。
    assert!(!has_layout_work(&tree));
}

#[test]
fn removing_overlay_owner_subtree_invalidates_old_surface_pixels() {
    // 用中间页面节点包裹窗口浮动按钮，模拟条件页面整棵卸载。
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        Container::new().size(400.0, 300.0),
        vec![ViewNode::new(
            Container::new(),
            vec![ViewNode::leaf(
                FloatButton::new("message-circle").placement(Placement::BottomRight),
            )],
        )],
    ));
    let root = tree.root_id().expect("根容器必须存在");
    let page = tree.get(root).expect("根容器必须可读").children()[0];
    let button = tree.get(page).expect("页面节点必须可读").children()[0];
    tree.get_mut(root)
        .expect("根容器必须可写")
        .set_frame(crate::core::Rect::new(0.0, 0.0, 400.0, 300.0));
    tree.layout();
    assert!(
        tree.overlay_stack()
            .iter()
            .any(|entry| entry.owner() == button)
    );

    tree.reset_invalidation();
    tree.remove(page);

    assert!(
        !tree
            .overlay_stack()
            .iter()
            .any(|entry| entry.owner() == button)
    );
    let invalidation = tree
        .invalidation()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    assert!(invalidation.node_needs_paint(root));
    drop(invalidation);
    assert!(tree.dirty_region().full_frame);
}
