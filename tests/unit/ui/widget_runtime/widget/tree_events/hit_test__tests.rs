// 引入被测 WidgetTree 命中排序私有实现。
use super::*;
// 使用长离场动画保持节点处于 pending-removal 状态。
use crate::ui::animation::AnimationConfig;
// 使用普通文本节点建立可命中的重叠兄弟。
use crate::ui::widgets::Label;

// 构造两个完全重叠且声明顺序稳定的子节点。
fn overlapping_children() -> (WidgetTree, WidgetId, WidgetId) {
    // 创建窗口私有运行时树。
    let mut tree = WidgetTree::new();
    // 根节点只负责拥有两个测试子节点。
    let root = tree.set_root(Box::new(Label::new("root")));
    // 后续坐标必须位于根可命中范围内。
    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 100.0, 100.0));
    // 先声明第一个兄弟。
    let first = tree.add_child(root, Box::new(Label::new("first")));
    // 后声明第二个兄弟。
    let second = tree.add_child(root, Box::new(Label::new("second")));
    // 两个兄弟使用完全相同的命中矩形。
    for child in [first, second] {
        // 让 z-order 成为唯一选择依据。
        tree.set_frame_dirty(child, Rect::new(0.0, 0.0, 80.0, 80.0));
    }
    // 返回树和稳定兄弟身份。
    (tree, first, second)
}

// 验证无分配排序仍保持原二维命中优先级。
#[test]
fn reusable_hit_test_sort_preserves_z_and_later_sibling_precedence() {
    // 建立两个重叠兄弟。
    let (mut tree, first, second) = overlapping_children();
    // 同 z-index 时后声明兄弟位于视觉上层。
    assert_eq!(tree.hit_test(Point::new(10.0, 10.0)), Some(second));
    // 更高 z-index 必须覆盖声明顺序。
    tree.set_z_index(first, 10);
    // 先声明兄弟提升后成为命中目标。
    assert_eq!(tree.hit_test(Point::new(10.0, 10.0)), Some(first));
}

// 验证高频命中在首次扩容后复用相同工作区容量。
#[test]
fn repeated_hit_test_reuses_tree_owned_sort_capacity() {
    // 建立会触发非空子节点排序的树。
    let (tree, _first, second) = overlapping_children();
    // 首次命中允许工作区按实际树宽度扩容。
    assert_eq!(tree.hit_test(Point::new(10.0, 10.0)), Some(second));
    // 记录预热后的容量与已恢复的逻辑长度。
    let warmed_capacity = {
        // 借用窗口树唯一拥有的排序工作区。
        let scratch = tree.hit_test_order_scratch.borrow();
        // 每次顶层调用结束都必须释放逻辑内容。
        assert!(scratch.is_empty());
        // 非空兄弟排序必须已经建立可复用容量。
        assert!(scratch.capacity() >= 2);
        scratch.capacity()
    };
    // 重复高频命中必须返回相同目标。
    assert_eq!(tree.hit_test(Point::new(10.0, 10.0)), Some(second));
    // 第二次调用不得增长或替换工作区。
    let scratch = tree.hit_test_order_scratch.borrow();
    assert!(scratch.is_empty());
    assert_eq!(scratch.capacity(), warmed_capacity);
}

// 验证任意子树命中入口与递归后代都继承祖先的待移除门控。
#[test]
fn pending_removal_ancestor_blocks_public_and_subtree_hit_tests() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Label::new("root")));
    let parent = tree.add_child(root, Box::new(Label::new("parent")));
    let leaf = tree.add_child(parent, Box::new(Label::new("leaf")));
    for id in [root, parent, leaf] {
        tree.set_frame_dirty(id, Rect::new(0.0, 0.0, 100.0, 100.0));
    }
    let point = Point::new(10.0, 10.0);
    assert_eq!(tree.hit_test(point), Some(leaf));

    tree.get_mut(parent)
        .expect("待离场父节点必须存在")
        .set_leave_animation(Some(AnimationConfig::fade_out(10.0)));
    assert!(tree.start_leave_transition(parent));

    // 公开根命中必须跳过整个离场子树并回落到仍可交互的根节点。
    assert_eq!(tree.hit_test(point), Some(root));
    // overlay 等私有调用可从任意后代进入，仍必须检查完整祖先链。
    assert_eq!(tree.hit_test_internal(leaf, point), None);
}
