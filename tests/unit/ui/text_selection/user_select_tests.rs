// 引入被测树级文字选择协调入口。
use super::*;
// 引入声明树到运行时树的唯一适配器。
use crate::ui::adapter::ViewAdapter;
// 引入最小容器、文字组件、声明节点与公开选择策略。
use crate::ui::{Container, Label, Typography, UserSelect, ViewNode};

// 构建一个容器下的 Label 与 Typography 并返回实际节点身份。
fn build_text_tree(policy: UserSelect) -> (WidgetTree, WidgetId, WidgetId, WidgetId) {
    // 创建默认不可选 Label，用于验证 text/all 能力覆盖。
    let label = ViewNode::leaf(Label::new("甲"));
    // 创建默认可选 Typography，用于验证 none 祖先关闭。
    let typography = ViewNode::leaf(Typography::text("乙"));
    // 根容器声明当前待测选择策略。
    let root = ViewNode::new(Container::new(), vec![label, typography])
        // 通过公开 View 入口保存结构策略。
        .user_select(policy);
    // 创建空实际树。
    let mut tree = WidgetTree::default();
    // 通过正式适配器挂载完整声明树。
    let root_id = tree.build(ViewAdapter::expand(root));
    // 复制两个直接文字子节点身份。
    let children = tree
        // 根节点必须存在。
        .get(root_id)
        // 测试树构造失败应立即报错。
        .expect("根节点应存在")
        // 读取文档序直接子节点。
        .children()
        // 复制为测试拥有集合。
        .to_vec();
    // 返回树、根与两个文字节点。
    (tree, root_id, children[0], children[1])
}

// none 应关闭整个子树，text 应启用默认不可选 Label。
#[test]
fn propagates_none_and_text_to_text_components() {
    // 构建禁止选择的文本子树。
    let (none_tree, _, none_label, none_typography) = build_text_tree(UserSelect::None);
    // 默认不可选 Label 在 none 下继续不参与。
    assert!(!none_tree.get(none_label).is_some_and(participates));
    // 默认可选 Typography 也必须被祖先 none 关闭。
    assert!(!none_tree.get(none_typography).is_some_and(participates));
    // 构建显式启用文本选择的子树。
    let (text_tree, _, text_label, text_typography) = build_text_tree(UserSelect::Text);
    // text 必须启用默认不可选 Label。
    assert!(text_tree.get(text_label).is_some_and(participates));
    // Typography 在 text 下保持可选。
    assert!(text_tree.get(text_typography).is_some_and(participates));
}

// all 应选择最近声明子树中的全部文本并按文档序聚合。
#[test]
fn selects_and_aggregates_nearest_all_subtree() {
    // 创建整体边界中的直接 Label。
    let label_view = ViewNode::leaf(Label::new("甲"));
    // 把 Typography 放入额外容器，验证 all 不限于直接子节点。
    let nested = ViewNode::new(
        // 使用非文字容器形成一层真实树嵌套。
        Container::new(),
        // 保存一个嵌套排版组件。
        vec![ViewNode::leaf(Typography::text("乙"))],
    );
    // 根容器声明整体选择策略并承载两个层级的文本。
    let root = ViewNode::new(Container::new(), vec![label_view, nested])
        // 最近 all 声明拥有完整根子树。
        .user_select(UserSelect::All);
    // 建立实际运行时树。
    let mut tree = WidgetTree::default();
    // 挂载完整嵌套声明并取得边界身份。
    let root_id = tree.build(ViewAdapter::expand(root));
    // 读取直接 Label 身份作为用户交互锚点。
    let label = tree
        // 根节点必须已经挂载。
        .get(root_id)
        // 测试树构建失败应立即报错。
        .expect("all 根节点应存在")
        // 第一个直接子节点是 Label。
        .children()[0];
    // 在 Label 上启动整体选择必须成功。
    assert!(tree.select_all_user_select_subtree(label));
    // 聚合内容必须包含两个组件并使用视觉行分隔。
    assert_eq!(
        // 从实际交互锚点读取完整选区。
        tree.aggregate_cross_text_selection(label),
        // 保持 Label 到 Typography 的声明顺序。
        Some("甲\n乙".to_string())
    );
}

// 协调切换到 none 应立即清除旧选择而无需重新创建组件。
#[test]
fn reconcile_policy_change_clears_existing_selection() {
    // 先以 text 启用两个文字参与者。
    let (mut tree, root, label, _) = build_text_tree(UserSelect::Text);
    // 直接建立 Label 完整范围模拟已有用户选区。
    if let Some(node) = tree.get(label) {
        // 选择当前单字符文本。
        set_range(node, Some((0, 1)));
    }
    // 旧范围必须可以聚合。
    assert_eq!(
        tree.aggregate_cross_text_selection(label),
        Some("甲".to_string())
    );
    // 同一实际根切换到 none 并传播到既有后代。
    tree.set_node_user_select(root, UserSelect::None);
    // 目标不再参与选择，因此旧范围不可观察。
    assert_eq!(tree.aggregate_cross_text_selection(label), None);
    // 组件自身旧选区也必须被策略切换清除。
    let selected = tree
        // 读取原 Label 实际节点。
        .get(label)
        // 下转型到具体组件以检查局部状态。
        .and_then(|node| node.component().as_any().downcast_ref::<Label>())
        // 读取拥有型选中文本。
        .and_then(Label::selected_text);
    // 禁止策略不得残留旧高亮范围。
    assert_eq!(selected, None);
}
