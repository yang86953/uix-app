// 导入展示树、树选择器及其公开模型，覆盖单 capability 的 prelude 公开面。
use uix::prelude::{DropPosition, SnapshotTreeNode, Tree, TreeNode, TreeSelect};

// 提供可执行 fixture 的稳定入口。
fn main() {
    // 构造包含子节点的公开树模型，证明节点 builder 可达。
    let root = TreeNode::new("根节点", "root").add(TreeNode::new("子节点", "child"));
    // 从同一节点生成公开快照，证明快照模型与组件同步启用。
    let snapshot = SnapshotTreeNode::from_tree_node(&root);
    // 构造展示树并消费拖拽位置模型，覆盖展示侧完整公开面。
    let tree = Tree::new(vec![root.clone()]).on_drop(|_, _, position| {
        // 显式匹配公开枚举，避免位置类型只停留在导入层。
        let _inside = matches!(position, DropPosition::Inside);
    });
    // 构造树选择器并复用节点模型，证明输入侧与展示侧共享同一 capability。
    let tree_select = TreeSelect::new().nodes(vec![root]);
    // 显式消费全部值，避免无意义的未使用警告。
    drop((snapshot, tree, tree_select));
}
