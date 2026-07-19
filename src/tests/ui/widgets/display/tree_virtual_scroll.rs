use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::display::tree::TREE_ROW_HEIGHT;
use crate::ui::widgets::display::tree::*;

fn large_tree() -> Tree {
    let nodes: Vec<TreeNode> = (0..80)
        .map(|i| TreeNode::new(&format!("Node {i}"), &format!("n-{i}")))
        .collect();
    Tree::new(nodes)
}

#[test]
fn tree_scroll_range_limits_visible_nodes() {
    let tree = large_tree();
    tree.last_frame.set(Some(Rect::new(0.0, 0.0, 200.0, 112.0)));
    let viewport_h = tree.body_viewport_height();
    let (start, end) = tree
        .body_scroll
        .scroll_range(tree.flat.len(), TREE_ROW_HEIGHT, viewport_h);
    assert_eq!(start, 0);
    assert!(
        end - start < 80,
        "virtual scroll should expose a small window"
    );
}

#[test]
fn tree_wheel_records_composite_delta() {
    let mut tree = large_tree();
    tree.last_frame.set(Some(Rect::new(0.0, 0.0, 200.0, 112.0)));

    assert_eq!(
        EventHandler::on_event(
            &mut tree,
            &SystemEvent::Wheel {
                pos: Point::new(10.0, 60.0),
                delta: Point::new(0.0, 1.0),
            },
        ),
        EventResult::Handled
    );
    assert!(tree.body_scroll.scroll_offset() > 0.0);
    assert_eq!(
        EventHandler::scroll_delta_for_dirty(&tree),
        Some((0.0, 40.0))
    );
}

#[test]
fn tree_wheel_registers_composite_scroll_strip() {
    let mut widget_tree = WidgetTree::new();
    let tree = large_tree();
    tree.last_frame.set(Some(Rect::new(0.0, 0.0, 200.0, 112.0)));
    let id = widget_tree.set_root(Box::new(tree));
    widget_tree
        .get_mut(id)
        .expect("tree root")
        .set_frame(Rect::new(0.0, 0.0, 200.0, 112.0));
    widget_tree.reset_invalidation();

    assert_eq!(
        widget_tree.dispatch_event(&SystemEvent::Wheel {
            pos: Point::new(20.0, 60.0),
            delta: Point::new(0.0, 1.0),
        }),
        EventResult::Handled
    );

    let moves = widget_tree
        .scroll_region_moves()
        .expect("tree scroll should register memmove");
    assert_eq!(moves.len(), 1);
    let (frame, dx, dy) = moves[0];
    assert_eq!(frame, Rect::new(0.0, 0.0, 200.0, 112.0));
    assert_eq!(dx, 0.0);
    assert_eq!(dy, 40.0);
}
