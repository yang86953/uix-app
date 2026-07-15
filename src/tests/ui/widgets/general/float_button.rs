use crate::tests::common::*;
use crate::ui::view::{embed, ViewAdapter};
use crate::ui::widgets::general::float_button::*;
use crate::ui::{AccessibilityRole, EventHandler, WidgetRender};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn measure_preserves_float_button_zero_layout_footprint() {
    let measured = FloatButton::new("+").measure(Constraints::loose(Size::new(40.0, 40.0)));

    assert_eq!(measured, Size::zero());
}

#[test]
fn float_button_exposes_real_hit_damage_overlay_and_accessibility_bounds() {
    let button = FloatButton::new("+")
        .tooltip("新建")
        .size(48.0)
        .position(12.0, 8.0);
    let frame = Rect::new(100.0, 50.0, 0.0, 0.0);
    let hit = EventHandler::hit_test_frame(&button, frame);
    assert_eq!(hit, Rect::new(112.0, 58.0, 48.0, 48.0));
    assert!(WidgetRender::dirty_rect(&button, frame).contains(Point::new(115.0, 60.0)));
    let overlay =
        WidgetRender::overlay_entry(&button, ComponentId::new(4), frame).expect("float overlay");
    assert_eq!(overlay.kind(), crate::ui::OverlayKind::Custom);
    assert_eq!(overlay.bounds_rect(), Some(hit));
    assert_eq!(
        button.snapshot_fields().accessibility().role,
        AccessibilityRole::Button
    );

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(button));
    tree.get_mut(id).expect("float root").set_frame(frame);
    assert_eq!(tree.visible_rect_for(id), Some(hit));
}

#[test]
fn pointer_and_keyboard_activation_reach_public_click_handler() {
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let mut tree = ViewAdapter::build(embed(FloatButton::new("+").tooltip("新建")).on_click_fn(
        move || {
            observed.set(observed.get() + 1);
        },
    ));
    let id = tree.root_id().expect("float root");
    tree.get_mut(id)
        .expect("float node")
        .set_frame(Rect::zero());
    let pos = Point::new(20.0, 20.0);

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 1);

    tree.set_focus(Some(id));
    assert_eq!(tree.get(id).expect("float node").component().tab_index(), 1);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 2);
}
