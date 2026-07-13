use crate::tests::common::*;
use crate::ui::widgets::display::tree::TreeNode;
use crate::ui::widgets::input::tree_select::*;

fn large_tree_select() -> TreeSelect {
    let nodes: Vec<TreeNode> = (0..80)
        .map(|i| TreeNode::new(&format!("Node {i}"), &format!("n-{i}")))
        .collect();
    TreeSelect::new().nodes(nodes)
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
    assert!(
        end - start < 80,
        "virtual scroll should expose a small window"
    );
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
