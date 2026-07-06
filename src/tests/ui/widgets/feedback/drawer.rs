use super::*;
use crate::core::Rect;
use crate::ui::traits::{WidgetAnimation, WidgetCapabilities, WidgetComponent};
use crate::ui::{WidgetCore, WidgetTree};

#[test]
fn drawer_advertises_animation_capability() {
    let drawer = Drawer::new("Drawer").show();

    assert!(drawer
        .capabilities()
        .contains(WidgetCapabilities::ANIMATION));
    assert!(drawer.as_animation().is_some());
}

#[test]
fn drawer_enter_transition_advances_and_marks_paint_dirty() {
    let mut drawer = Drawer::new("Drawer").show();
    let initial_offset = drawer.transition.offset;

    assert!(WidgetAnimation::update_animation(&mut drawer, 0.05));
    assert_ne!(drawer.transition.offset, initial_offset);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Drawer::new("Drawer").show()));
    tree.get_mut(id)
        .expect("drawer root")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.invalidation().lock().unwrap().clear();

    assert!(tree.update(1.0 / 60.0));

    let queue = tree.invalidation().lock().unwrap();
    assert!(queue.has_paint_or_composite());
    assert!(queue.node_needs_paint(id));
}

#[test]
fn drawer_close_finishes_exit_transition_before_internal_hide() {
    let mut drawer = Drawer::new("Drawer").show();
    assert!(!WidgetAnimation::update_animation(&mut drawer, 1.0));
    assert!(drawer.is_visible());

    drawer.close();
    assert!(!drawer.is_visible());
    assert!(drawer.is_present());

    assert!(!WidgetAnimation::update_animation(&mut drawer, 1.0));
    assert!(!drawer.is_present());
    assert!(drawer.transition_dirty);
}
