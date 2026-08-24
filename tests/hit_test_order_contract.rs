//! 验证公开运行时命中行为在内部排序工作区复用后保持不变。

use uix::prelude::{Label, Point, Rect};
use uix::ui::__private::WidgetTree;

// 构造两个完全重叠、声明顺序稳定的普通子节点。
fn overlapping_children() -> (WidgetTree, uix::prelude::WidgetId, uix::prelude::WidgetId) {
    // 创建独立窗口树。
    let mut tree = WidgetTree::new();
    // 根节点拥有测试兄弟。
    let root = tree.set_root(Box::new(Label::new("root")));
    // 根几何覆盖目标坐标。
    tree.set_frame_dirty(root, Rect::new(0.0, 0.0, 100.0, 100.0));
    // 按稳定声明顺序添加两个兄弟。
    let first = tree.add_child(root, Box::new(Label::new("first")));
    let second = tree.add_child(root, Box::new(Label::new("second")));
    // 两个兄弟使用相同命中矩形。
    for child in [first, second] {
        tree.set_frame_dirty(child, Rect::new(0.0, 0.0, 80.0, 80.0));
    }
    // 返回树与兄弟身份。
    (tree, first, second)
}

#[test]
fn hit_test_preserves_z_index_and_later_sibling_precedence() {
    // 建立完全重叠的兄弟节点。
    let (mut tree, first, second) = overlapping_children();
    // 同 z-index 时后声明兄弟位于视觉上层。
    assert_eq!(tree.hit_test(Point::new(10.0, 10.0)), Some(second));
    // 显式提高先声明兄弟的 z-index。
    tree.set_z_index(first, 10);
    // 更高 z-index 必须覆盖声明顺序。
    assert_eq!(tree.hit_test(Point::new(10.0, 10.0)), Some(first));
}
