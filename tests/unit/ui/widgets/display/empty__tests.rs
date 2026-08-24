//! Empty UIX 视觉声明回归测试。

use super::*;

/// UIX 视觉声明必须保持原 Empty 类型、配置和单叶节点形状。
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
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Empty>()
        .expect("UIX 根必须保留 Empty 内核");
    assert_eq!(kernel.visual.min_width, 160.0);
    assert_eq!(kernel.visual.max_width, 320.0);
    assert_eq!(kernel.visual.text_font_size, 13.0);
    assert_eq!(kernel.visual.icon_size, 32.0);
}

/// 单次描述布局必须同时返回显式多行的行数与对应行盒高度。
#[test]
fn description_layout_returns_count_and_height_together() {
    let (line_count, height) = Empty::new().description_layout("第一行\n第二行", 1000.0);
    assert_eq!(line_count, 2);
    assert_eq!(height, 39.0);
}
