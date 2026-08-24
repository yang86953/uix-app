use crate::ui::adapter::ViewAdapter;
use crate::ui::widget_runtime::widget::WidgetCore;
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
fn all_visible_layout_skips_filtered_child_allocation() {
    // 构造常见的全可见容器子树，覆盖布局收敛中的统一子项排列入口。
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        Container::new().size(320.0, 200.0),
        vec![
            ViewNode::leaf(Space::new().width(24.0).height(16.0)),
            ViewNode::leaf(Space::new().width(32.0).height(20.0)),
        ],
    ));
    let root = tree.root_id().expect("根容器必须存在");
    tree.get_mut(root)
        .expect("根容器必须可写")
        .set_frame(crate::core::Rect::new(0.0, 0.0, 320.0, 200.0));

    crate::ui::widget_runtime::widget::boxed::reset_filtered_child_allocation_count();
    tree.layout();

    assert_eq!(
        crate::ui::widget_runtime::widget::boxed::filtered_child_allocation_count(),
        0,
        "全可见热路径不得为过滤子项分配临时 Vec",
    );
}

#[test]
fn hidden_child_layout_keeps_filtered_fallback() {
    // 构造一个隐藏子项，确认稀有过滤路径仍然保留既有可见性语义。
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        Container::new().size(320.0, 200.0),
        vec![
            ViewNode::leaf(Space::new().width(24.0).height(16.0)),
            ViewNode::leaf(Space::new().width(32.0).height(20.0)),
        ],
    ));
    let root = tree.root_id().expect("根容器必须存在");
    let hidden = tree.get(root).expect("根容器必须可读").children()[1];
    tree.get_mut(root)
        .expect("根容器必须可写")
        .set_frame(crate::core::Rect::new(0.0, 0.0, 320.0, 200.0));
    tree.set_visible(hidden, false);

    crate::ui::widget_runtime::widget::boxed::reset_filtered_child_allocation_count();
    tree.layout();

    assert!(
        crate::ui::widget_runtime::widget::boxed::filtered_child_allocation_count() > 0,
        "隐藏子树仍应进入过滤回退路径",
    );
    assert_eq!(tree.get(hidden).expect("隐藏子项必须存在").frame().w, 0.0);
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
