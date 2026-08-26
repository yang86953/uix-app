//! 验证 WidgetTree 单次槽位查询仍完整执行作用域、代际与占用校验。

use uix::prelude::Label;
use uix::ui::__private::WidgetTree;

#[test]
fn tree_rejects_cross_scope_and_recycled_widget_identities() {
    // 第一棵树创建可回收的真实子节点。
    let mut first_tree = WidgetTree::new();
    let first_root = first_tree.set_root(Box::new(Label::new("first-root")));
    let removed = first_tree.add_child(first_root, Box::new(Label::new("removed")));

    // 第二棵树即使拥有相同槽号和代际，也不得接纳第一棵树的身份。
    let mut second_tree = WidgetTree::new();
    second_tree.set_root(Box::new(Label::new("second-root")));
    assert!(second_tree.get(removed).is_none());

    // 移除会释放槽位并推进代际，旧身份必须立即失效。
    first_tree.remove(removed);
    assert!(first_tree.get(removed).is_none());

    // 后续节点复用同一物理槽位，但只能由新代际身份访问。
    let replacement = first_tree.add_child(first_root, Box::new(Label::new("replacement")));
    assert_eq!(replacement.slot(), removed.slot());
    assert_ne!(replacement.generation(), removed.generation());
    assert!(first_tree.get(removed).is_none());
    assert!(first_tree.get(replacement).is_some());
}

#[test]
fn root_replacement_invalidates_the_previous_root_identity() {
    // 首次根节点占用零号槽位。
    let mut tree = WidgetTree::new();
    let previous_root = tree.set_root(Box::new(Label::new("previous")));

    // 完整换根重用槽位并推进代际，旧根不得命中新节点。
    let current_root = tree.set_root(Box::new(Label::new("current")));
    assert_eq!(current_root.slot(), previous_root.slot());
    assert_ne!(current_root.generation(), previous_root.generation());
    assert!(tree.get(previous_root).is_none());
    assert!(tree.get(current_root).is_some());
}
