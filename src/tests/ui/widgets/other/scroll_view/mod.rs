use super::*;
use crate::core::{ComponentId, Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::layout::{AlignItems, FlexDirection};
use crate::ui::traits::{EventHandler, WidgetCapabilities, WidgetLayout, WidgetRender};
use crate::ui::widgets::{Collapse, CollapsePanel, Container, Space};
use crate::ui::EventResult;
use std::cell::RefCell;
use std::rc::Rc;

struct FixedWidget {
    size: Size,
    #[allow(dead_code)]
    id: ComponentId,
}

impl WidgetComponent for FixedWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER)
    }

    crate::wc_upcast!(FixedWidget; WidgetLayout);
    crate::wc_upcast!(FixedWidget; WidgetRender);
}

impl WidgetLayout for FixedWidget {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.size)
    }
}

impl WidgetRender for FixedWidget {
    fn render(&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}
}

struct MeasureProbeWidget {
    size: Size,
    seen: Rc<RefCell<Vec<Constraints>>>,
}

impl WidgetComponent for MeasureProbeWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT)
    }

    crate::wc_upcast!(MeasureProbeWidget; WidgetLayout);
}

impl WidgetLayout for MeasureProbeWidget {
    fn measure(&self, constraints: Constraints) -> Size {
        self.seen.borrow_mut().push(constraints);
        constraints.clamp(self.size)
    }
}

#[test]
fn scrollview_scroll_to() {
    let sv = ScrollView::new(ScrollDirection::Vertical).scroll_to(0.0, 100.0);
    assert_eq!(sv.scroll_x, 0.0);
    assert_eq!(sv.scroll_y, 100.0);
}

#[test]
fn measure_clamps_scrollview_size() {
    let measured = ScrollView::new(ScrollDirection::Vertical)
        .size(300.0, 200.0)
        .measure(Constraints::loose(Size::new(120.0, 90.0)));

    assert_eq!(measured, Size::new(120.0, 90.0));
}

#[test]
fn scrollview_scroll_to_clamped() {
    let sv = ScrollView::new(ScrollDirection::Vertical).scroll_to(-10.0, -50.0);
    assert_eq!(sv.scroll_x, 0.0);
    assert_eq!(sv.scroll_y, 0.0);
}

#[test]
fn scrollview_set_scroll_programmatically() {
    let mut sv = ScrollView::new(ScrollDirection::Both);
    sv.set_scroll_x(50.0);
    sv.set_scroll_y(75.0);
    assert_eq!(sv.scroll_x, 50.0);
    assert_eq!(sv.scroll_y, 75.0);
}

#[test]
fn scrollview_programmatic_scroll_records_composite_delta() {
    let mut sv = ScrollView::new(ScrollDirection::Both);
    sv.set_scroll_x(50.0);
    sv.set_scroll_y(75.0);

    assert_eq!(sv.scroll_delta_for_dirty(), Some((50.0, 75.0)));
    assert!(sv.scroll_delta_for_dirty().is_none());
}

#[test]
fn scrollview_keyboard_page_scroll_records_composite_delta() {
    let mut sv = ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0);
    sv.content_bounds.set(Some(Size::new(300.0, 600.0)));
    sv.last_frame.set(Some(Rect::new(0.0, 0.0, 300.0, 200.0)));

    assert_eq!(
        sv.on_event(&SystemEvent::KeyDown {
            key: KeyCode::PageDown,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );

    assert_eq!(sv.scroll_y(), 180.0);
    assert_eq!(sv.scroll_delta_for_dirty(), Some((0.0, 180.0)));
    assert!(sv.scroll_delta_for_dirty().is_none());
}

#[test]
fn scrollview_keyboard_home_end_records_actual_clamped_delta() {
    let mut sv = ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0);
    sv.content_bounds.set(Some(Size::new(300.0, 600.0)));
    sv.last_frame.set(Some(Rect::new(0.0, 0.0, 300.0, 200.0)));

    assert_eq!(
        sv.on_event(&SystemEvent::KeyDown {
            key: KeyCode::End,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(sv.scroll_y(), 400.0);
    assert_eq!(sv.scroll_delta_for_dirty(), Some((0.0, 400.0)));

    assert_eq!(
        sv.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Home,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(sv.scroll_y(), 0.0);
    assert_eq!(sv.scroll_delta_for_dirty(), Some((0.0, -400.0)));
}

#[test]
fn scrollview_child_builder() {
    let sv = ScrollView::new(ScrollDirection::Vertical).child(FixedWidget {
        size: Size::new(100.0, 200.0),
        id: ComponentId::new(0),
    });
    assert!(sv.children.is_set());
    assert_eq!(sv.children.len(), 1);
    let children = sv.children.take();
    assert_eq!(children.len(), 1);
}

#[test]
fn scrollview_layout_children_uses_natural_coordinates() {
    let scrollview = ScrollView::new(ScrollDirection::Vertical)
        .size(200.0, 300.0)
        .scroll_to(0.0, 50.0);

    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(scrollview));
    tree.add_child(
        root_id,
        Box::new(FixedWidget {
            size: Size::new(200.0, 600.0),
            id: ComponentId::new(1),
        }),
    );

    tree.layout();

    let frame = tree.get(root_id).map(|n| n.frame()).unwrap_or_default();
    let children = tree
        .get(root_id)
        .map(|n| n.children().to_vec())
        .unwrap_or_default();
    let result = tree
        .get(root_id)
        .unwrap()
        .layout_children(frame, &children, &tree);

    let (_, rect) = result.first().expect("expected child rect");
    assert_eq!(rect.x, frame.x);
    assert_eq!(rect.y, frame.y);
}

#[test]
fn scrollview_vertical_fills_viewport_width_even_when_child_measures_narrower() {
    let mut tree = WidgetTree::new();
    let vertical = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).size(400.0, 200.0),
    ));
    tree.add_child(
        vertical,
        Box::new(FixedWidget {
            size: Size::new(120.0, 80.0),
            id: ComponentId::new(9),
        }),
    );
    tree.layout();
    let child = tree.get(vertical).unwrap().children()[0];
    assert_eq!(
        tree.get(child).unwrap().frame(),
        Rect::new(0.0, 0.0, 400.0, 80.0),
        "vertical ScrollView must stretch child to viewport width"
    );
}

#[test]
fn scrollview_child_constraints_are_axis_aware() {
    let mut tree = WidgetTree::new();
    let vertical = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).size(120.0, 80.0),
    ));
    tree.add_child(
        vertical,
        Box::new(FixedWidget {
            size: Size::new(400.0, 300.0),
            id: ComponentId::new(1),
        }),
    );
    tree.layout();
    let vertical_child = tree.get(vertical).unwrap().children()[0];
    assert_eq!(
        tree.get(vertical_child).unwrap().frame(),
        Rect::new(0.0, 0.0, 120.0, 300.0)
    );
    let vertical_sv: &ScrollView = tree
        .get(vertical)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref()
        .unwrap();
    assert_eq!(vertical_sv.max_scroll_x(), 0.0);
    assert_eq!(vertical_sv.max_scroll_y(), 220.0);

    let horizontal = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Horizontal).size(120.0, 80.0),
    ));
    tree.add_child(
        horizontal,
        Box::new(FixedWidget {
            size: Size::new(400.0, 300.0),
            id: ComponentId::new(2),
        }),
    );
    tree.layout();
    let horizontal_child = tree.get(horizontal).unwrap().children()[0];
    assert_eq!(
        tree.get(horizontal_child).unwrap().frame(),
        Rect::new(0.0, 0.0, 400.0, 80.0)
    );
    let horizontal_sv: &ScrollView = tree
        .get(horizontal)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref()
        .unwrap();
    assert_eq!(horizontal_sv.max_scroll_x(), 280.0);
    assert_eq!(horizontal_sv.max_scroll_y(), 0.0);
}

fn assert_two_stage_measurement(
    direction: ScrollDirection,
    expected_constraints: Constraints,
    expected_rect: Rect,
) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut tree = WidgetTree::new();
    let scroll_view = tree.set_root(Box::new(ScrollView::new(direction).size(120.0, 80.0)));
    let child = tree.add_child(
        scroll_view,
        Box::new(MeasureProbeWidget {
            size: Size::new(400.0, 300.0),
            seen: Rc::clone(&seen),
        }),
    );
    let frame = Rect::new(0.0, 0.0, 120.0, 80.0);
    let scroll_view: &ScrollView = tree
        .get(scroll_view)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref()
        .unwrap();

    let measured = scroll_view.measure_children(frame, &[child], &tree);
    assert_eq!(&*seen.borrow(), &[expected_constraints]);
    assert_eq!(measured.len(), 1);

    let placements = scroll_view.layout_children(frame, &measured, &tree);
    assert_eq!(placements, vec![(child, expected_rect)]);
    assert_eq!(
        seen.borrow().len(),
        1,
        "arrange must consume LayoutChild.measured_size without re-measuring"
    );
}

#[test]
fn scrollview_two_stage_measurement_is_axis_aware_and_arrange_does_not_measure() {
    assert_two_stage_measurement(
        ScrollDirection::Vertical,
        Constraints::loose(Size::new(120.0, f32::MAX)),
        Rect::new(0.0, 0.0, 120.0, 300.0),
    );
    assert_two_stage_measurement(
        ScrollDirection::Horizontal,
        Constraints::loose(Size::new(f32::MAX, 80.0)),
        Rect::new(0.0, 0.0, 400.0, 80.0),
    );
    assert_two_stage_measurement(
        ScrollDirection::Both,
        Constraints::loose(Size::new(f32::MAX, f32::MAX)),
        Rect::new(0.0, 0.0, 400.0, 300.0),
    );
}

#[test]
fn scrollview_multi_child_flow_respects_direction_and_non_scroll_axis() {
    let mut horizontal_tree = WidgetTree::new();
    let horizontal = horizontal_tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Horizontal).size(120.0, 80.0),
    ));
    horizontal_tree.add_child(
        horizontal,
        Box::new(FixedWidget {
            size: Size::new(70.0, 30.0),
            id: ComponentId::new(10),
        }),
    );
    horizontal_tree.add_child(
        horizontal,
        Box::new(FixedWidget {
            size: Size::new(90.0, 40.0),
            id: ComponentId::new(11),
        }),
    );
    horizontal_tree.layout();

    let horizontal_children = horizontal_tree.get(horizontal).unwrap().children().to_vec();
    assert_eq!(
        horizontal_tree.get(horizontal_children[0]).unwrap().frame(),
        Rect::new(0.0, 0.0, 70.0, 80.0)
    );
    assert_eq!(
        horizontal_tree.get(horizontal_children[1]).unwrap().frame(),
        Rect::new(70.0, 0.0, 90.0, 80.0)
    );
    let horizontal_sv: &ScrollView = horizontal_tree
        .get(horizontal)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref()
        .unwrap();
    assert_eq!(horizontal_sv.max_scroll_x(), 40.0);
    assert_eq!(horizontal_sv.max_scroll_y(), 0.0);

    let mut vertical_tree = WidgetTree::new();
    let vertical = vertical_tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).size(120.0, 80.0),
    ));
    vertical_tree.add_child(
        vertical,
        Box::new(FixedWidget {
            size: Size::new(30.0, 30.0),
            id: ComponentId::new(12),
        }),
    );
    vertical_tree.add_child(
        vertical,
        Box::new(FixedWidget {
            size: Size::new(40.0, 70.0),
            id: ComponentId::new(13),
        }),
    );
    vertical_tree.layout();

    let vertical_children = vertical_tree.get(vertical).unwrap().children().to_vec();
    assert_eq!(
        vertical_tree.get(vertical_children[0]).unwrap().frame(),
        Rect::new(0.0, 0.0, 120.0, 30.0)
    );
    assert_eq!(
        vertical_tree.get(vertical_children[1]).unwrap().frame(),
        Rect::new(0.0, 30.0, 120.0, 70.0)
    );
    let vertical_sv: &ScrollView = vertical_tree
        .get(vertical)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref()
        .unwrap();
    assert_eq!(vertical_sv.max_scroll_x(), 0.0);
    assert_eq!(vertical_sv.max_scroll_y(), 20.0);
}

#[test]
fn scrollview_both_direction_allows_both_axes_to_overflow() {
    let mut tree = WidgetTree::new();
    let both = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Both).size(120.0, 80.0),
    ));
    tree.add_child(
        both,
        Box::new(FixedWidget {
            size: Size::new(400.0, 300.0),
            id: ComponentId::new(3),
        }),
    );

    tree.layout();

    let child = tree.get(both).unwrap().children()[0];
    assert_eq!(
        tree.get(child).unwrap().frame(),
        Rect::new(0.0, 0.0, 400.0, 300.0)
    );
    let both_sv: &ScrollView = tree
        .get(both)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref()
        .unwrap();
    assert_eq!(both_sv.max_scroll_x(), 280.0);
    assert_eq!(both_sv.max_scroll_y(), 220.0);
}

fn assert_nested_content_axis_caps(
    direction: ScrollDirection,
    expected_frame: Rect,
    expected_max_scroll: (f32, f32),
) {
    let mut tree = WidgetTree::new();
    let scroll_view = tree.set_root(Box::new(ScrollView::new(direction).size(120.0, 80.0)));
    let content = tree.add_child(
        scroll_view,
        Box::new(
            Container::new()
                .dir(FlexDirection::Column)
                .align(AlignItems::Start)
                .overflow_content(),
        ),
    );
    tree.add_child(
        content,
        Box::new(FixedWidget {
            size: Size::new(400.0, 300.0),
            id: ComponentId::new(4),
        }),
    );

    tree.layout();

    assert_eq!(
        tree.get(content).unwrap().frame(),
        expected_frame,
        "nested content frame must follow {direction:?} axis caps"
    );
    let scroll_view: &ScrollView = tree
        .get(scroll_view)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref()
        .unwrap();
    assert_eq!(
        (scroll_view.max_scroll_x(), scroll_view.max_scroll_y()),
        expected_max_scroll,
        "nested content bounds must follow {direction:?} axis caps"
    );
}

#[test]
fn scrollview_nested_content_respects_directional_parent_caps() {
    assert_nested_content_axis_caps(
        ScrollDirection::Vertical,
        Rect::new(0.0, 0.0, 120.0, 300.0),
        (0.0, 220.0),
    );
    assert_nested_content_axis_caps(
        ScrollDirection::Horizontal,
        Rect::new(0.0, 0.0, 400.0, 80.0),
        (280.0, 0.0),
    );
    assert_nested_content_axis_caps(
        ScrollDirection::Both,
        Rect::new(0.0, 0.0, 400.0, 300.0),
        (280.0, 220.0),
    );
}

#[test]
fn scrollview_direction_flags() {
    assert!(ScrollDirection::Vertical.can_scroll_y());
    assert!(!ScrollDirection::Vertical.can_scroll_x());
    assert!(ScrollDirection::Horizontal.can_scroll_x());
    assert!(!ScrollDirection::Horizontal.can_scroll_y());
    assert!(ScrollDirection::Both.can_scroll_x());
    assert!(ScrollDirection::Both.can_scroll_y());
}

#[test]
fn scrollview_not_handled_for_non_scrollbar_pointer_down() {
    let mut sv = ScrollView::new(ScrollDirection::Vertical);
    let result = sv.on_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 10.0),
        button: crate::ui::MouseButton::Left,
        mods: crate::native::traits::input::KeyMod::NONE,
    });
    assert_eq!(result, EventResult::NotHandled);
}

#[test]
fn scrollview_wheel_registers_composite_scroll_strip() {
    let mut tree = WidgetTree::new();
    let sv_id = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0),
    ));
    tree.add_child(
        sv_id,
        Box::new(FixedWidget {
            size: Size::new(300.0, 600.0),
            id: ComponentId::new(1),
        }),
    );

    tree.layout();
    tree.get_mut(sv_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<ScrollView>()
        .unwrap()
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 300.0, 200.0)));
    tree.reset_invalidation();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::Wheel {
            pos: Point::new(20.0, 20.0),
            delta: Point::new(0.0, 1.0),
        }),
        EventResult::Handled
    );

    let dirty = tree.dirty_region();
    assert!(!dirty.full_frame);
    assert_eq!(dirty.rects().len(), 1);
    let strip = dirty.rects()[0];
    assert!((strip.x - 0.0).abs() < 1e-6);
    assert!((strip.y - 150.0).abs() < 1e-6);
    assert!((strip.w - 300.0).abs() < 1e-6);
    assert!((strip.h - 50.0).abs() < 1e-6);

    let (frame, dx, dy) = tree
        .drain_scroll_region_move()
        .expect("scroll memmove should be registered");
    assert_eq!(frame, Rect::new(0.0, 0.0, 300.0, 200.0));
    assert_eq!(dx, 0.0);
    assert_eq!(dy, 50.0);

    let sv = tree
        .get(sv_id)
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<ScrollView>()
        .unwrap();
    assert!(sv.scroll_delta_for_dirty().is_none());
}

#[test]
fn scrollview_wheel_at_scroll_boundary_does_not_fallback_invalidate() {
    let mut tree = WidgetTree::new();
    let sv_id = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0),
    ));
    tree.add_child(
        sv_id,
        Box::new(FixedWidget {
            size: Size::new(300.0, 600.0),
            id: ComponentId::new(1),
        }),
    );

    tree.layout();
    let sv = tree
        .get_mut(sv_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<ScrollView>()
        .unwrap();
    sv.last_frame.set(Some(Rect::new(0.0, 0.0, 300.0, 200.0)));
    sv.scroll_y = sv.max_scroll_y();
    sv.scroll_delta_strip.set((0.0, 0.0));
    tree.reset_invalidation();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::Wheel {
            pos: Point::new(20.0, 20.0),
            delta: Point::new(0.0, 1.0),
        }),
        EventResult::NotHandled
    );

    let dirty = tree.dirty_region();
    assert!(!dirty.full_frame);
    assert!(dirty.rects().is_empty());
    assert!(tree.drain_scroll_region_move().is_none());
}

#[test]
fn scrollview_keyboard_page_scroll_registers_composite_scroll_strip() {
    let mut tree = WidgetTree::new();
    let sv_id = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0),
    ));
    tree.add_child(
        sv_id,
        Box::new(FixedWidget {
            size: Size::new(300.0, 600.0),
            id: ComponentId::new(1),
        }),
    );

    tree.layout();
    tree.get_mut(sv_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<ScrollView>()
        .unwrap()
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 300.0, 200.0)));
    tree.managers_mut().focus.set_focused_component(Some(sv_id));
    tree.reset_invalidation();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::PageDown,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );

    let (frame, dx, dy) = tree
        .drain_scroll_region_move()
        .expect("keyboard scroll should register memmove");
    assert_eq!(frame, Rect::new(0.0, 0.0, 300.0, 200.0));
    assert_eq!(dx, 0.0);
    assert_eq!(dy, 180.0);

    let dirty = tree.dirty_region();
    assert!(!dirty.full_frame);
    assert_eq!(dirty.rects(), &[Rect::new(0.0, 20.0, 300.0, 180.0)]);
}

#[test]
fn scrollview_scrollbar_drag_registers_composite_scroll_strip() {
    let mut tree = WidgetTree::new();
    let sv_id = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0),
    ));
    tree.add_child(
        sv_id,
        Box::new(FixedWidget {
            size: Size::new(300.0, 600.0),
            id: ComponentId::new(1),
        }),
    );

    tree.layout();
    tree.get_mut(sv_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<ScrollView>()
        .unwrap()
        .last_frame
        .set(Some(Rect::new(0.0, 0.0, 300.0, 200.0)));
    tree.reset_invalidation();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(295.0, 10.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    tree.reset_invalidation();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerMove {
            pos: Point::new(295.0, 18.0),
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );

    let (frame, dx, dy) = tree
        .drain_scroll_region_move()
        .expect("scrollbar drag should register memmove");
    assert_eq!(frame, Rect::new(0.0, 0.0, 300.0, 200.0));
    assert_eq!(dx, 0.0);
    assert!(dy > 0.0 && dy < 200.0, "unexpected drag delta {dy}");

    let dirty = tree.dirty_region();
    assert!(!dirty.full_frame);
    assert_eq!(dirty.rects(), &[Rect::new(0.0, 200.0 - dy, 300.0, dy)]);
}

#[test]
fn scrollview_programmatic_scroll_invalidates_composite_strip() {
    let mut tree = WidgetTree::new();
    let sv_id = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0),
    ));
    tree.add_child(
        sv_id,
        Box::new(FixedWidget {
            size: Size::new(300.0, 600.0),
            id: ComponentId::new(1),
        }),
    );

    tree.layout();
    tree.reset_invalidation();
    tree.get_mut(sv_id)
        .unwrap()
        .component_mut()
        .as_any_mut()
        .downcast_mut::<ScrollView>()
        .unwrap()
        .set_scroll_y(50.0);

    tree.invalidate_paint(sv_id);

    let dirty = tree.dirty_region();
    assert!(!dirty.full_frame);
    assert_eq!(dirty.rects().len(), 1);
    assert_eq!(dirty.rects()[0], Rect::new(0.0, 150.0, 300.0, 50.0));

    let (frame, dx, dy) = tree
        .drain_scroll_region_move()
        .expect("programmatic scroll should register memmove");
    assert_eq!(frame, Rect::new(0.0, 0.0, 300.0, 200.0));
    assert_eq!(dx, 0.0);
    assert_eq!(dy, 50.0);
}

#[test]
fn scrollview_expand_child_updates_content_bounds() {
    struct GrowWidget {
        size: std::cell::Cell<f32>,
    }

    impl WidgetComponent for GrowWidget {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
            self
        }

        fn capabilities(&self) -> WidgetCapabilities {
            WidgetCapabilities::from_bits(
                WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER | WidgetCapabilities::EVENT,
            )
        }

        crate::wc_upcast!(GrowWidget; WidgetLayout);
        crate::wc_upcast!(GrowWidget; WidgetRender);
        crate::wc_upcast!(GrowWidget; EventHandler);
    }

    impl WidgetLayout for GrowWidget {
        fn measure(&self, constraints: Constraints) -> Size {
            constraints.clamp(Size::new(300.0, self.size.get()))
        }
    }

    impl WidgetRender for GrowWidget {
        fn render(&self, _: Rect, _: &mut PaintContext, _: &WidgetTree) {}
    }

    impl EventHandler for GrowWidget {
        fn on_event(&mut self, event: &SystemEvent) -> EventResult {
            if matches!(event, SystemEvent::PointerDown { .. }) {
                self.size.set(self.size.get() * 2.0);
                EventResult::Handled
            } else {
                EventResult::NotHandled
            }
        }
    }

    let mut tree = WidgetTree::new();
    let sv_id = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0),
    ));
    tree.add_child(
        sv_id,
        Box::new(GrowWidget {
            size: std::cell::Cell::new(100.0),
        }),
    );

    tree.layout();
    let max_y = |tree: &WidgetTree| -> f32 {
        let sv = tree.get(sv_id).unwrap();
        let sv_ref: &ScrollView = sv.component().as_any().downcast_ref().unwrap();
        sv_ref.max_scroll_y()
    };
    assert_eq!(max_y(&tree), 0.0);

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 10.0),
        button: crate::ui::MouseButton::Left,
        mods: crate::native::traits::input::KeyMod::NONE,
    });
    tree.layout();
    assert_eq!(max_y(&tree), 0.0);

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 10.0),
        button: crate::ui::MouseButton::Left,
        mods: crate::native::traits::input::KeyMod::NONE,
    });
    tree.layout();
    let max = max_y(&tree);
    assert!(
        (max - 200.0).abs() < 1.0,
        "expected max_scroll_y near 200, got {max}"
    );
}

#[test]
fn collapse_expand_updates_scrollview_content_bounds() {
    let mut tree = WidgetTree::new();
    let sv_id = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).size(300.0, 200.0),
    ));
    let container_id = tree.add_child(
        sv_id,
        Box::new(Container::new().size(300.0, 0.0).dir(FlexDirection::Column)),
    );
    let space_id = tree.add_child(
        container_id,
        Box::new(
            Space::new()
                .width(300.0)
                .height(140.0)
                .direction(FlexDirection::Column)
                .align(AlignItems::Stretch),
        ),
    );
    let long_content = "line\nline\nline\nline\nline\nline\nline";
    tree.add_child(
        space_id,
        Box::new(Collapse::new().panels(vec![
            CollapsePanel::new("Panel A", "short"),
            CollapsePanel::new("Panel B", long_content),
            CollapsePanel::new("Panel C", "short"),
        ])),
    );

    tree.layout();
    let max_before = tree
        .get(sv_id)
        .and_then(|n| {
            n.component()
                .as_any()
                .downcast_ref::<ScrollView>()
                .map(|sv| sv.max_scroll_y())
        })
        .unwrap();
    assert_eq!(max_before, 0.0);

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 45.0),
        button: crate::ui::MouseButton::Left,
        mods: crate::native::traits::input::KeyMod::NONE,
    });
    tree.layout();
    let max_after = tree
        .get(sv_id)
        .and_then(|n| {
            n.component()
                .as_any()
                .downcast_ref::<ScrollView>()
                .map(|sv| sv.max_scroll_y())
        })
        .unwrap();
    let content_height = tree.get(container_id).unwrap().frame().h;

    assert!(
        max_after > max_before,
        "expected max_scroll_y to increase after expand, before={max_before}, after={max_after}"
    );
    assert!(
        content_height > 200.0,
        "expanded content must exceed the vertical viewport, got {content_height}"
    );
}
