//! Empty UIX 声明壳回归测试。

use super::*;

/// UIX 声明壳必须保持原 Empty 类型、配置和单叶节点形状。
#[test]
fn uix_shell_preserves_single_empty_kernel_leaf() {
    let node = View::build(Empty::new().description("暂无数据").icon("inbox"));
    assert!(node.children.is_empty());
    assert!(node.widget.as_any().is::<Empty>());
    assert_eq!(
        node.widget.snapshot_fields(),
        SnapshotFields::Empty {
            description: "暂无数据".to_owned(),
            icon_name: "inbox".to_owned(),
            image: String::new(),
        }
    );
}

/// 单次描述布局必须同时返回显式多行的行数与对应行盒高度。
#[test]
fn description_layout_returns_count_and_height_together() {
    let (line_count, height) = Empty::description_layout("第一行\n第二行", 1000.0);
    assert_eq!(line_count, 2);
    assert_eq!(height, 39.0);
}
