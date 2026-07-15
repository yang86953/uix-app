use crate::tests::common::*;
use crate::ui::widgets::display::tree::TreeNode;
use crate::ui::widgets::{
    AutoComplete, Cascader, CascaderOption, ColorPicker, Mentions, Select, TreeSelect,
};

fn assert_popup_hit<T: EventHandler>(widget: &T, frame: Rect, point: Point) {
    let hit_frame = widget.hit_test_frame(frame);
    assert_ne!(hit_frame, frame);
    assert!(
        hit_frame.contains(point),
        "popup point {point:?} outside {hit_frame:?}"
    );
}

#[test]
fn selection_popups_expand_real_tree_hit_testing() {
    let frame = Rect::new(40.0, 30.0, 200.0, 32.0);
    let popup_point = Point::new(50.0, 74.0);

    let mut select = Select::new().options(["Alpha", "Beta"]);
    assert_eq!(select.hit_test_frame(frame), frame);
    select.open();
    assert_popup_hit(&select, frame, popup_point);

    let mut autocomplete = AutoComplete::new().options(vec!["Alpha", "Beta"]);
    autocomplete.open();
    assert_popup_hit(&autocomplete, frame, popup_point);

    let mut mentions = Mentions::new("Mention").options(vec!["Alpha", "Beta"]);
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "@a".to_string(),
    });
    assert_popup_hit(&mentions, frame, popup_point);

    let mut cascader = Cascader::new(vec![CascaderOption::new("Alpha", "alpha")], "Region");
    cascader.open();
    assert_popup_hit(&cascader, frame, popup_point);

    let mut tree_select = TreeSelect::new().nodes(vec![TreeNode::new("Alpha", "alpha")]);
    tree_select.open();
    assert_popup_hit(&tree_select, frame, popup_point);

    let mut color_picker = ColorPicker::new();
    color_picker.open();
    assert_popup_hit(
        &color_picker,
        Rect::new(40.0, 30.0, 32.0, 32.0),
        popup_point,
    );

    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Select::new().options(["Alpha", "Beta"])));
    tree.get_mut(root).expect("select root").set_frame(frame);
    tree.get_mut(root)
        .expect("select root")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Select>()
        .expect("Select component")
        .open();
    assert_eq!(tree.hit_test(popup_point), Some(root));
}
