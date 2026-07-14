use crate::tests::common::*;
use crate::ui::foundation::virtual_scroll::*;
use crate::ui::view::{label, ViewAdapter};
use crate::ui::widgets::{Button, Label};
use std::cell::Cell;
use std::rc::Rc;

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
fn default_overscan_matches_public_contract() {
    let vs = VirtualScroll::new().item_count(20).item_height(32.0);

    assert_eq!(vs.scroll_range(64.0), (0, 7));
}

#[test]
fn build_visible_children_uses_renderer() {
    let builder = VirtualScroll::new()
        .item_count(10)
        .item_height(32.0)
        .overscan(1)
        .size(200.0, 64.0)
        .render(|i| label(format!("row-{i}")));
    let mut tree = ViewAdapter::build(builder);
    let root = tree.root_id().expect("virtual scroll root");
    tree.layout();

    assert!(tree.has_virtual_scroll_renderer(root));
    let labels = tree.find_all_by_type::<Label>();
    assert!(!labels.is_empty());
    assert!(labels.len() <= 4);
}

#[test]
fn virtual_scroll_renderer_is_released_with_component_sidecar() {
    let capture = Rc::new(Cell::new(0));
    let renderer_capture = Rc::clone(&capture);
    let builder = VirtualScroll::new()
        .item_count(20)
        .item_height(32.0)
        .render(move |i| {
            renderer_capture.set(renderer_capture.get() + 1);
            label(format!("row-{i}"))
        });
    let mut tree = ViewAdapter::build(builder);
    let root = tree.root_id().expect("virtual scroll root");

    assert!(tree.has_virtual_scroll_renderer(root));
    assert!(capture.get() > 0);
    assert_eq!(Rc::strong_count(&capture), 2);

    tree.remove(root);
    assert_eq!(Rc::strong_count(&capture), 1);
}

#[test]
fn reconcile_replaces_virtual_scroll_renderer_sidecar() {
    let first_capture = Rc::new(Cell::new(0));
    let first_renderer = Rc::clone(&first_capture);
    let mut tree = ViewAdapter::build(
        VirtualScroll::new()
            .item_count(20)
            .item_height(20.0)
            .size(200.0, 60.0)
            .render(move |i| {
                first_renderer.set(first_renderer.get() + 1);
                label(format!("first-{i}"))
            }),
    );
    let root = tree.root_id().expect("virtual scroll root");

    assert!(first_capture.get() > 0);
    assert_eq!(Rc::strong_count(&first_capture), 2);

    let second_capture = Rc::new(Cell::new(0));
    let second_renderer = Rc::clone(&second_capture);
    ViewAdapter::reconcile(
        &mut tree,
        VirtualScroll::new()
            .item_count(20)
            .item_height(20.0)
            .size(200.0, 60.0)
            .render(move |i| {
                second_renderer.set(second_renderer.get() + 1);
                label(format!("second-{i}"))
            }),
    );

    assert_eq!(tree.root_id(), Some(root));
    assert!(tree.has_virtual_scroll_renderer(root));
    assert_eq!(Rc::strong_count(&first_capture), 1);
    assert!(second_capture.get() > 0);
    assert_eq!(Rc::strong_count(&second_capture), 2);
}

#[test]
fn replacing_tree_root_drops_virtual_scroll_renderer() {
    let capture = Rc::new(Cell::new(0));
    let renderer_capture = Rc::clone(&capture);
    let mut tree = ViewAdapter::build(VirtualScroll::new().item_count(10).render(move |i| {
        renderer_capture.set(renderer_capture.get() + 1);
        label(format!("row-{i}"))
    }));
    assert_eq!(Rc::strong_count(&capture), 2);

    ViewAdapter::reconcile(&mut tree, label("replacement"));
    assert_eq!(Rc::strong_count(&capture), 1);
}

#[test]
fn widget_tree_build_auto_prepares_visible_rows() {
    let builder = VirtualScroll::new()
        .item_count(50)
        .item_height(32.0)
        .size(200.0, 96.0)
        .render(|i| label(format!("row-{i}")));
    let mut tree = ViewAdapter::build(builder);
    let root_id = tree.root_id().expect("virtual scroll root");
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
    vs.mark_children_materialized((5, 8));
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

#[test]
fn same_type_reconcile_preserves_virtual_scroll_offset() {
    let initial = VirtualScroll::new()
        .item_count(100)
        .item_height(20.0)
        .size(200.0, 80.0)
        .render(|i| label(format!("row-{i}")));
    let mut tree = ViewAdapter::build(initial);
    tree.layout();
    let root = tree.root_id().expect("virtual scroll root");
    tree.reset_invalidation();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::Wheel {
            pos: crate::core::Point::new(10.0, 10.0),
            delta: crate::core::Point::new(0.0, -1.0),
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.get(root)
            .and_then(|node| node.component().as_any().downcast_ref::<VirtualScroll>())
            .map(VirtualScroll::scroll_offset),
        Some(40.0)
    );

    let next = VirtualScroll::new()
        .item_count(120)
        .item_height(20.0)
        .size(200.0, 80.0)
        .render(|i| label(format!("next-{i}")));
    ViewAdapter::reconcile(&mut tree, next);

    assert_eq!(tree.root_id(), Some(root));
    assert_eq!(
        tree.get(root)
            .and_then(|node| node.component().as_any().downcast_ref::<VirtualScroll>())
            .map(VirtualScroll::scroll_offset),
        Some(40.0)
    );
}

#[test]
fn prelude_render_materializes_interactive_view_rows() {
    use crate::prelude::{button, VirtualScroll as PreludeVirtualScroll};

    let clicks = Rc::new(Cell::new(0));
    let renderer_clicks = Rc::clone(&clicks);
    let mut tree = ViewAdapter::build(
        PreludeVirtualScroll::new()
            .item_count(1)
            .item_height(32.0)
            .size(200.0, 64.0)
            .render(move |index| {
                let button_clicks = Rc::clone(&renderer_clicks);
                button(format!("row-{index}")).on_click_fn(move || {
                    button_clicks.set(button_clicks.get() + 1);
                })
            }),
    );
    let root = tree.root_id().expect("virtual scroll root");
    tree.get_mut(root)
        .expect("virtual scroll node")
        .set_frame(Rect::new(0.0, 0.0, 200.0, 64.0));
    tree.layout();

    let button_id = tree
        .find_all_by_type::<Button>()
        .first()
        .map(|(id, _)| *id)
        .expect("materialized button row");
    let frame = tree.get(button_id).expect("button row node").frame();
    let click = Point::new(frame.x + frame.w * 0.5, frame.y + frame.h * 0.5);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: click,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: click,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(clicks.get(), 1);
}
