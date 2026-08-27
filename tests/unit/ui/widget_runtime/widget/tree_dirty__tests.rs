// 引入被测树级失效实现与私有辅助函数。
use super::*;
use crate::core::WidgetId;
use crate::ui::reactive::state::{State, StatePaintBind};
use crate::ui::widgets::{Container, Space};
use std::sync::Arc;

// 构造带根与直接子节点的最小真实组件树，并清除建树阶段的失效。
fn tree_with_root_and_child() -> (WidgetTree, WidgetId, WidgetId) {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new()));
    let child = tree.add_child(root, Box::new(Space::new()));
    tree.reset_invalidation();
    (tree, root, child)
}

// 读取共享队列条目，避免测试在持锁期间继续调用树方法。
fn invalidation_items(tree: &WidgetTree) -> Vec<Invalidation> {
    tree.invalidation()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .items
        .clone()
}

// 分数滚动无法由整数纹理搬移精确表达时必须回退为重绘。
#[test]
fn fractional_scroll_delta_is_not_rounded_into_texture_copy() {
    // 空树足以验证合成事务不会登记错误的 retained move。
    let mut tree = WidgetTree::new();
    let viewport = Rect::new(0.0, 0.0, 120.0, 80.0);
    // 十二点五像素若被取整，旧文字像素会与当前布局逐帧漂移。
    assert!(!tree.push_scroll_composite(viewport, 0.0, 12.5));
    assert_eq!(tree.scroll_region_moves(), None);
}

// 整数视口和整数位移仍应保留局部纹理搬移快路径。
#[test]
fn integral_scroll_delta_keeps_exact_texture_copy() {
    let mut tree = WidgetTree::new();
    let viewport = Rect::new(4.0, 6.0, 120.0, 80.0);
    assert!(tree.push_scroll_composite(viewport, 0.0, 12.0));
    assert_eq!(
        tree.scroll_region_moves(),
        Some(vec![ScrollCopy::new(viewport, 0.0, 12.0)])
    );
}

// 内容视口外的滚动条沟槽必须形成互不重叠的独立重绘区域。
#[test]
fn scroll_chrome_regions_preserve_right_and_bottom_gutters() {
    let frame = Rect::new(10.0, 20.0, 100.0, 80.0);
    let viewport = Rect::new(10.0, 20.0, 92.0, 72.0);
    assert_eq!(
        scroll_chrome_regions(frame, viewport),
        vec![
            Rect::new(10.0, 92.0, 100.0, 8.0),
            Rect::new(102.0, 20.0, 8.0, 72.0),
        ]
    );
}

// 非批次根 Layout 已存在时，子请求不再写入队列但必须返回无需传播。
#[test]
fn non_batch_root_layout_suppresses_child_request_and_advances_revision() {
    let (mut tree, root, child) = tree_with_root_and_child();
    assert!(tree.push_layout_invalidation(root));
    let before_revision = tree.invalidation_revision();

    assert!(!tree.push_layout_invalidation(child));
    assert_eq!(invalidation_items(&tree), vec![Invalidation::Layout(root)]);
    assert_eq!(
        tree.invalidation_revision(),
        before_revision.wrapping_add(1),
    );
}

// 首个子请求传播到根后，下一请求应从队列建立的根提示快返且不重复条目。
#[test]
fn propagated_root_layout_suppresses_followup_request() {
    let (mut tree, root, child) = tree_with_root_and_child();

    assert!(tree.push_layout_invalidation(child));
    tree.propagate_layout_invalidation(child);
    assert!(!tree.push_layout_invalidation(child));
    assert_eq!(
        invalidation_items(&tree),
        vec![Invalidation::Layout(child), Invalidation::Layout(root)]
    );
}

// 批次内即使根 Layout 已存在也必须写入 pending，直到结束才发布共享队列。
#[test]
fn batch_root_layout_keeps_child_pending_until_finish() {
    let (mut tree, root, child) = tree_with_root_and_child();
    assert!(tree.push_layout_invalidation(root));
    let published_before_batch = invalidation_items(&tree);

    tree.begin_invalidation_batch();
    assert!(tree.push_layout_invalidation(child));
    assert_eq!(invalidation_items(&tree), published_before_batch);
    assert_eq!(
        tree.pending_invalidations,
        vec![Invalidation::Layout(child)],
    );

    tree.finish_invalidation_batch();
    assert_eq!(
        invalidation_items(&tree),
        vec![Invalidation::Layout(root), Invalidation::Layout(child)],
    );
    assert!(tree.pending_invalidations.is_empty());
    assert_eq!(tree.invalidation_batch_depth, 0);
}

// 空输入必须释放现有节点租约，并接受带容量但无元素的 Vec。
#[test]
fn empty_node_state_binds_unbinds_existing_lease() {
    let (mut tree, _root, child) = tree_with_root_and_child();
    let state = State::new(1_u32);
    let source: Arc<dyn StatePaintBind> = Arc::new(state.clone());

    // 先建立一个真实节点租约，作为空替换的旧状态。
    tree.replace_node_captured_state_binds(child, vec![source]);
    assert_eq!(state.reconcile_sites.lock().unwrap().len(), 1);

    // 即使输入 Vec 保留容量，空快路径也必须精确解绑旧租约。
    tree.replace_node_captured_state_binds(child, Vec::with_capacity(8));
    assert!(state.reconcile_sites.lock().unwrap().is_empty());
}

// 已移除节点的空替换必须沿用 get_mut 门控并保持无副作用。
#[test]
fn empty_node_state_binds_ignores_removed_node() {
    let (mut tree, _root, child) = tree_with_root_and_child();
    let state = State::new(2_u32);
    let source: Arc<dyn StatePaintBind> = Arc::new(state.clone());

    // 让节点先持有一份租约，再通过正式移除路径释放它。
    tree.replace_node_captured_state_binds(child, vec![source]);
    tree.remove(child);
    assert!(state.reconcile_sites.lock().unwrap().is_empty());

    // stale WidgetId 传入空 Vec 不得重新绑定或触碰其他节点。
    tree.replace_node_captured_state_binds(child, Vec::with_capacity(8));
    assert!(state.reconcile_sites.lock().unwrap().is_empty());
}
