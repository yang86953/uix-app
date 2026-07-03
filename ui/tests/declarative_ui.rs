use uix_ui::{ui, Button, ButtonVariant, Container, Label};

#[test]
fn ui_macro_builds_leaf_widget() {
    let node = ui! { Button("保存", variant: ButtonVariant::Primary, danger: true) };

    assert!(node.widget.as_any().is::<Button>());
    assert!(node.children.is_empty());
}

#[test]
fn ui_macro_builds_parent_with_children() {
    let node = ui! {
        Container {
            Label("标题"),
            Button("提交", variant: ButtonVariant::Primary),
        }
    };

    assert!(node.widget.as_any().is::<Container>());
    assert_eq!(node.children.len(), 2);
    assert!(node.children[0].widget.as_any().is::<Label>());
    assert!(node.children[1].widget.as_any().is::<Button>());
}
