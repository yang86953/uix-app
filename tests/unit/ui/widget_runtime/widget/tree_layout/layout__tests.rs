use crate::ui::adapter::ViewAdapter;
use crate::ui::{Space, ViewNode};

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
