//! ResultView UIX 声明壳回归测试。

use super::*;

/// UIX 声明壳必须保持原 ResultView 类型、配置和单叶节点形状。
#[test]
fn uix_shell_preserves_single_result_kernel_leaf() {
    let node = View::build(
        ResultView::new(ResultType::NotFound)
            .title("页面不存在")
            .extra_text("返回首页"),
    );
    assert!(node.children.is_empty());
    assert!(node.widget.as_any().is::<ResultView>());
    assert_eq!(
        node.widget.snapshot_fields(),
        SnapshotFields::Result {
            result_type: ResultType::NotFound,
            title: "页面不存在".to_owned(),
            subtitle: String::new(),
            extra_text: "返回首页".to_owned(),
        }
    );
}

/// 常见单行动作文案必须保持借用，仅真实换行输入才归一化并分配。
#[test]
fn action_text_normalization_keeps_single_line_borrowed() {
    assert!(matches!(
        ResultView::normalized_action_text("返回首页"),
        std::borrow::Cow::Borrowed("返回首页")
    ));
    assert_eq!(
        ResultView::normalized_action_text("返回\n首页"),
        "返回 首页"
    );
}
