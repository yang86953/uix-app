use super::*;
use crate::ui::traits::{EventHandler, WidgetComponent, WidgetLayout};
use crate::ui::widgets::Label;
use crate::ui::WidgetTree;

#[test]
fn virtual_list_scroll_range_matches_virtual_scroll() {
    let vs = VirtualScroll::new()
        .item_count(100)
        .item_height(32.0)
        .overscan(2);
    let helper = VirtualListScroll::new().overscan(2);
    assert_eq!(vs.scroll_range(96.0), helper.scroll_range(100, 32.0, 96.0));
}

#[test]
fn scroll_range_includes_overscan() {
    let vs = VirtualScroll::new()
        .item_count(100)
        .item_height(32.0)
        .overscan(2);
    let (start, end) = vs.scroll_range(96.0);
    assert_eq!(start, 0);
    assert_eq!(end, 5);
}

#[test]
fn scroll_range_respects_item_count() {
    let vs = VirtualScroll::new()
        .item_count(5)
        .item_height(32.0)
        .overscan(3);
    let (start, end) = vs.scroll_range(300.0);
    assert_eq!(start, 0);
    assert_eq!(end, 5);
}

#[test]
fn build_visible_children_uses_renderer() {
    let vs = VirtualScroll::new()
        .item_count(10)
        .item_height(32.0)
        .overscan(1)
        .renderer(|i| WidgetNode::leaf(Box::new(Label::new(format!("row-{i}")))));
    let nodes = vs.build_visible_children(64.0);
    assert!(!nodes.is_empty());
    assert!(nodes.len() <= 4);
}

#[test]
fn prepare_for_build_populates_children() {
    let vs = VirtualScroll::new()
        .item_count(20)
        .item_height(32.0)
        .renderer(|i| WidgetNode::leaf(Box::new(Label::new(format!("row-{i}")))));
    vs.prepare_for_build(96.0);
    assert_eq!(vs.visible_start(), 0);
    let built = vs.build();
    assert!(!built.is_empty());
}

#[test]
fn widget_tree_build_auto_prepares_visible_rows() {
    use crate::ui::core::widget::{WidgetCore, WidgetNode};

    let vs = VirtualScroll::new()
        .item_count(50)
        .item_height(32.0)
        .size(200.0, 96.0)
        .renderer(|i| WidgetNode::leaf(Box::new(Label::new(format!("row-{i}")))));
    let mut tree = WidgetTree::new();
    let root_id = tree.build(WidgetNode::leaf(Box::new(vs)));
    tree.get_mut(root_id)
        .expect("virtual scroll root")
        .set_frame(Rect::new(0.0, 0.0, 200.0, 96.0));
    tree.layout();

    let labels = tree.find_all_by_type::<Label>();
    assert!(
        !labels.is_empty(),
        "VirtualScroll should mount visible rows without manual prepare_for_build"
    );
    assert!(
        labels.len() < 50,
        "virtual scroll should not mount the full list"
    );
}

#[test]
fn scroll_ratio_at_bounds() {
    let vs = VirtualScroll::new()
        .item_count(10)
        .item_height(32.0)
        .size(300.0, 96.0);
    assert_eq!(vs.scroll_ratio(96.0), 0.0);
}

#[test]
fn virtual_list_scroll_range_applies_overscan() {
    let mut scroll = VirtualListScroll::new().overscan(1);
    scroll.set_scroll_offset(64.0);

    assert_eq!(scroll.scroll_range(100, 32.0, 96.0), (1, 6));
}

#[test]
fn virtual_list_range_clamps_invalid_offsets_and_overscan_overflow() {
    assert_eq!(
        virtual_list_index_range(10, 20.0, f32::INFINITY, 40.0, 1),
        (0, 3)
    );
    assert_eq!(
        virtual_list_index_range(10, 20.0, f32::NAN, 40.0, usize::MAX),
        (0, 10)
    );
    assert_eq!(
        virtual_list_index_range(10, 20.0, f32::MAX, 40.0, 1),
        (10, 10)
    );

    let mut scroll = VirtualListScroll::new();
    scroll.set_scroll_offset(f32::INFINITY);
    assert_eq!(scroll.scroll_offset(), 0.0);
    scroll.set_scroll_offset(f32::NAN);
    assert_eq!(scroll.scroll_offset(), 0.0);
}

#[test]
fn virtual_list_scroll_wheel_delta_clamps_to_content() {
    let mut scroll = VirtualListScroll::new();

    assert_eq!(scroll.scroll_by_wheel(-10.0, 5, 20.0, 40.0), 60.0);
    assert_eq!(scroll.scroll_offset(), 60.0);
    assert_eq!(scroll.scroll_by_wheel(-1.0, 5, 20.0, 40.0), 0.0);
    assert_eq!(scroll.scroll_by_wheel(10.0, 5, 20.0, 40.0), -60.0);
    assert_eq!(scroll.scroll_offset(), 0.0);

    scroll.set_scroll_offset(80.0);
    scroll.clamp_to_content(3, 20.0, 40.0);
    assert_eq!(scroll.scroll_offset(), 20.0);
}

#[test]
fn measure_uses_configured_viewport_size() {
    let vs = VirtualScroll::new().size(240.0, 180.0);
    let size = vs.measure(Constraints::unconstrained());
    assert_eq!(size, Size::new(240.0, 180.0));
}

#[test]
fn layout_children_positions_by_absolute_index() {
    let vs = VirtualScroll::new().item_count(100).item_height(32.0);
    vs.visible_start.set(5);
    let frame = Rect::new(0.0, 0.0, 200.0, 96.0);
    let ids = [
        ComponentId::new(1),
        ComponentId::new(2),
        ComponentId::new(3),
    ];
    let children: Vec<_> = ids
        .iter()
        .copied()
        .map(|id| crate::ui::LayoutChild::new(id, Size::zero()))
        .collect();
    let positions = vs.layout_children(frame, &children, &WidgetTree::new());
    assert_eq!(positions.len(), 3);
    assert_eq!(positions[0].1.y, 160.0);
    assert_eq!(positions[1].1.y, 192.0);
}

#[test]
fn wheel_scroll_records_delta_for_composite() {
    let mut vs = VirtualScroll::new()
        .item_count(100)
        .item_height(32.0)
        .size(300.0, 96.0);
    vs.last_frame.set(Some(Rect::new(0.0, 0.0, 300.0, 96.0)));
    let handled = vs.on_event(&SystemEvent::Wheel {
        pos: crate::core::Point::new(0.0, 0.0),
        delta: crate::core::Point::new(0.0, -1.0),
    });
    assert_eq!(handled, EventResult::Handled);
    assert!(vs.scroll_offset() > 0.0);
    assert_eq!(vs.scroll_delta_for_dirty(), Some((0.0, 40.0)));
    assert!(vs.scroll_delta_for_dirty().is_none());
}
