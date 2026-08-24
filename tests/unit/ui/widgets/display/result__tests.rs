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
