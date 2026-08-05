// 故意导入未启用 capability 的展示树、树选择器与公开模型，门禁要求此行无法编译。
use uix::prelude::{DropPosition, SnapshotTreeNode, Tree, TreeNode, TreeSelect};

// 提供 compile-fail fixture 的稳定入口。
fn main() {
    // 若节点模型意外泄漏，构造值会使门禁检测到编译成功并失败。
    let node = TreeNode::new("不应可用", "disabled");
    // 若展示树意外泄漏，构造值会阻止部分门控静默通过。
    let tree = Tree::new(vec![node.clone()]);
    // 若树选择器意外泄漏，构造值会覆盖输入侧公开面。
    let tree_select = TreeSelect::new().nodes(vec![node.clone()]);
    // 若快照模型意外泄漏，转换调用会参与公开面门禁。
    let snapshot = SnapshotTreeNode::from_tree_node(&node);
    // 若拖拽位置模型意外泄漏，枚举值会参与公开面门禁。
    let position = DropPosition::Inside;
    // 显式消费全部值，避免公开面泄漏时只产生未使用警告。
    drop((tree, tree_select, snapshot, position));
}
