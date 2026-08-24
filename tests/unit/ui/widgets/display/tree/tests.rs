// 引入待验证的树组件与公开节点模型。
use super::{TREE_VISUAL_REF, Tree, TreeNode};
// 引入公开 View 构建入口以验证 UIX 根。
use crate::ui::view::View;

// 验证树级 checkable 递归覆盖完整节点层级。
#[test]
// 声明树级勾选能力测试。
fn tree_level_checkable_applies_to_all_nodes() {
    // 构造包含两层子树且节点初始能力不同的数据。
    let tree = Tree::new(vec![
        // 根节点初始不可勾选。
        TreeNode::new("根", "root").children(vec![
            // 子节点初始显式可勾选。
            TreeNode::new("子项", "child").checkable(true),
        ]),
    ])
    // 树级 true 必须覆盖所有层级。
    .checkable(true);
    // 根节点必须获得勾选入口。
    assert!(tree.nodes[0].checkable);
    // 嵌套节点必须获得相同勾选入口。
    assert!(tree.nodes[0].children[0].checkable);

    // 树级 false 同样必须递归关闭所有入口。
    let tree = tree.checkable(false);
    // 根节点必须关闭勾选入口。
    assert!(!tree.nodes[0].checkable);
    // 嵌套节点必须同步关闭勾选入口。
    assert!(!tree.nodes[0].children[0].checkable);
}

// 验证 default_expand_all 建立完整初始展开集合。
#[test]
// 声明默认全展开测试。
fn tree_default_expand_all_collects_every_expandable_key() {
    // 构造静态分支、嵌套分支与懒加载分支。
    let tree = Tree::new(vec![
        // 根分支包含嵌套分支。
        TreeNode::new("根", "root").children(vec![
            // 内部分支拥有一个叶节点。
            TreeNode::new("分支", "branch")
                // 建立静态子树。
                .children(vec![TreeNode::new("叶", "leaf")]),
        ]),
        // 懒加载节点即使尚无子项也具有展开语义。
        TreeNode::new("延迟", "lazy").lazy(true),
    ])
    // 启用初始全展开。
    .default_expand_all(true);
    // 展开集合必须保持声明顺序并包含全部可展开节点。
    assert_eq!(tree.expanded_keys, vec!["root", "branch", "lazy"]);
    // 扁平列表必须包含已经展开的嵌套叶节点。
    assert!(tree.flat.iter().any(|node| node.key == "leaf"));
}

// 验证声明式刷新不会用默认值重置用户展开状态。
#[test]
// 声明默认值生命周期所有权测试。
fn tree_default_expand_all_does_not_control_reconciled_state() {
    // 首次物化时展开根节点。
    let mut current = Tree::new(vec![
        // 根节点提供一个可见子项。
        TreeNode::new("根", "root").children(vec![TreeNode::new("子项", "child")]),
    ])
    // 声明默认全展开。
    .default_expand_all(true);
    // 模拟用户在运行时折叠根节点。
    current.set_expanded("root", false);
    // 后续声明式刷新仍携带同一个默认值。
    let next = Tree::new(vec![
        // 使用同一稳定键重建声明数据。
        TreeNode::new("根", "root").children(vec![TreeNode::new("子项", "child")]),
    ])
    // 默认值只描述首次物化。
    .default_expand_all(true);
    // 执行真实组件同步路径。
    current.sync_from(next);
    // 用户折叠状态必须继续由运行时拥有。
    assert!(!current.expanded_keys.iter().any(|key| key == "root"));
}

// 验证声明式刷新保留用户改变的节点勾选状态。
#[test]
// 声明勾选状态生命周期所有权测试。
fn tree_checkable_refresh_preserves_runtime_checked_state() {
    // 首次物化一棵可勾选树。
    let mut current = Tree::new(vec![TreeNode::new("根", "root")]).checkable(true);
    // 模拟用户通过运行时交互勾选根节点。
    current.toggle_check("root");
    // 后续声明式刷新继续声明树级勾选能力。
    let next = Tree::new(vec![TreeNode::new("根", "root")]).checkable(true);
    // 执行真实组件同步路径。
    current.sync_from(next);
    // 用户勾选事实必须继续由运行时状态拥有。
    assert!(current.nodes[0].checked);
}

// 验证 UIX 根保留节点配置并注入真实视觉契约。
#[test]
fn uix_root_preserves_tree_kernel_and_visual_contract() {
    let node = View::build(
        Tree::new(vec![TreeNode::new("根", "root")])
            .searchable(true)
            .multiple(true),
    );
    assert!(node.children.is_empty());
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Tree>()
        .expect("UIX 根必须保留 Tree 内核");
    assert_eq!(kernel.nodes.len(), 1);
    assert!(kernel.searchable);
    assert!(kernel.multiple);
    assert_eq!(
        kernel.visual_contract_for_test(),
        (200.0, 300.0, 28.0, 32.0, 20.0, 13.0)
    );
}

// 验证单次递归筛选只保留命中路径且维持声明顺序。
#[test]
fn search_filter_keeps_only_matching_paths() {
    let mut tree = Tree::new(vec![
        TreeNode::new("根", "root").children(vec![
            TreeNode::new("目标叶", "target"),
            TreeNode::new("普通叶", "other"),
        ]),
        TreeNode::new("旁支", "aside"),
    ])
    .searchable(true);
    tree.set_search_query("目标");
    assert_eq!(tree.visible_keys_for_test(), vec!["root", "target"]);
}

// 验证无分配键盘移动仍跳过禁用节点并在边界停留。
#[test]
fn keyboard_selection_skips_disabled_nodes_and_clamps() {
    let mut tree = Tree::new(vec![
        TreeNode::new("禁用", "disabled").disabled(true),
        TreeNode::new("甲", "alpha"),
        TreeNode::new("乙", "beta"),
    ]);
    tree.move_selection(true);
    assert_eq!(tree.selected_key(), "alpha");
    tree.move_selection(true);
    tree.move_selection(true);
    assert_eq!(tree.selected_key(), "beta");
    tree.move_selection(false);
    assert_eq!(tree.selected_key(), "alpha");
}

// 验证全部 Tree 实例共享同一份 UIX 视觉表。
#[test]
fn tree_instances_share_uix_visual_table() {
    let first = Tree::new(Vec::new());
    assert!(std::ptr::eq(first.visual, TREE_VISUAL_REF));
    let first = View::build(first);
    let second = View::build(Tree::new(Vec::new()));
    let first = first.widget.as_any().downcast_ref::<Tree>().unwrap();
    let second = second.widget.as_any().downcast_ref::<Tree>().unwrap();
    assert!(first.shares_visual_with_for_test(second));
    assert!(std::ptr::eq(first.visual, TREE_VISUAL_REF));
}
