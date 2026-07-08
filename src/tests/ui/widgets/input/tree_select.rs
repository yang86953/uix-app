use super::*;
use crate::core::{Constraints, Point, Size};
use crate::ui::traits::{EventHandler, WidgetAnimation, WidgetLayout};
use crate::ui::{EventResult, SystemEvent};
use crate::ui::widgets::display::TreeNode;

fn large_tree_select() -> TreeSelect {
    let nodes: Vec<TreeNode> = (0..80)
        .map(|i| TreeNode::new(&format!("Node {i}"), &format!("n-{i}")))
        .collect();
    TreeSelect::new().nodes(nodes)
}

#[test]
fn tree_select_enter_animation_finishes_open() {
    let mut tree_select = TreeSelect::new().nodes(vec![TreeNode::new("Alpha", "alpha")]);

    tree_select.open();
    assert!(tree_select.is_open());
    assert!(tree_select.is_present());

    assert!(WidgetAnimation::update_animation(&mut tree_select, 0.05));
    assert!(!WidgetAnimation::update_animation(&mut tree_select, 1.0));
    assert!(tree_select.is_open());
    assert!(tree_select.is_present());
}

#[test]
fn tree_select_exit_animation_stays_present_until_finished() {
    let mut tree_select = TreeSelect::new().nodes(vec![TreeNode::new("Alpha", "alpha")]);
    tree_select.open();
    assert!(!WidgetAnimation::update_animation(&mut tree_select, 1.0));

    tree_select.close();
    assert!(!tree_select.is_open());
    assert!(tree_select.is_present());

    assert!(WidgetAnimation::update_animation(&mut tree_select, 0.03));
    assert!(!WidgetAnimation::update_animation(&mut tree_select, 1.0));
    assert!(!tree_select.is_open());
    assert!(!tree_select.is_present());
}

#[test]
fn measure_clamps_tree_select_size() {
    let measured = TreeSelect::new()
        .nodes(vec![TreeNode::new("Alpha", "alpha")])
        .measure(Constraints::loose(Size::new(120.0, 20.0)));

    assert_eq!(measured, Size::new(120.0, 20.0));
}

#[test]
fn tree_select_dropdown_scroll_range_limits_visible_rows() {
    let tree_select = large_tree_select();
    let row_count = tree_select.flatten_nodes().len();
    let viewport_h = tree_select.dropdown_viewport_height(row_count);
    let (start, end) = tree_select
        .dropdown_scroll
        .scroll_range(row_count, 28.0, viewport_h);
    assert_eq!(start, 0);
    assert!(end - start < 80, "virtual scroll should expose a small window");
}

#[test]
fn tree_select_dropdown_wheel_records_composite_delta() {
    let mut tree_select = large_tree_select();
    tree_select.open();

    assert_eq!(
        EventHandler::on_event(
            &mut tree_select,
            &SystemEvent::Wheel {
                pos: Point::new(10.0, 50.0),
                delta: Point::new(0.0, -1.0),
            },
        ),
        EventResult::Handled
    );
    assert!(tree_select.dropdown_scroll.scroll_offset() > 0.0);
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&tree_select),
        Some((0.0, 40.0))
    );
}

#[test]
fn tree_select_dropdown_row_at_y_accounts_for_scroll_offset() {
    let mut tree_select = large_tree_select();
    tree_select.open();
    tree_select.dropdown_scroll.set_scroll_offset(28.0 * 5.0);

    assert_eq!(tree_select.dropdown_row_at_y(33.0), Some(5));
    assert_eq!(tree_select.dropdown_row_at_y(61.0), Some(6));
}
