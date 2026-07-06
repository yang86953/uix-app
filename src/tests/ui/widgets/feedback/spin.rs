use crate::core::Rect;
use crate::ui::traits::{WidgetAnimation, WidgetCapabilities, WidgetComponent};
use crate::ui::{Spin, WidgetCore, WidgetTree};

#[test]
fn spin_advertises_animation_capability() {
    let spin = Spin::new();

    assert!(spin.capabilities().contains(WidgetCapabilities::ANIMATION));
    assert!(spin.as_animation().is_some());
}

#[test]
fn spinning_spin_advances_phase_and_marks_paint_dirty() {
    let mut spin = Spin::new();
    let initial_phase = spin.phase();

    assert!(WidgetAnimation::update_animation(&mut spin, 0.25));
    assert_ne!(spin.phase(), initial_phase);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Spin::new()));
    tree.get_mut(id)
        .expect("spin root")
        .set_frame(Rect::new(4.0, 5.0, 24.0, 24.0));
    tree.invalidation().lock().unwrap().clear();

    assert!(tree.update(1.0 / 60.0));

    let queue = tree.invalidation().lock().unwrap();
    assert!(queue.has_paint_or_composite());
    assert!(queue.node_needs_paint(id));
}

#[test]
fn stopped_spin_does_not_keep_animation_pending() {
    let mut spin = Spin::new().spinning(false);
    let initial_phase = spin.phase();

    assert!(!WidgetAnimation::update_animation(&mut spin, 0.25));
    assert_eq!(spin.phase(), initial_phase);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Spin::new().spinning(false)));
    tree.get_mut(id)
        .expect("spin root")
        .set_frame(Rect::new(4.0, 5.0, 24.0, 24.0));
    tree.invalidation().lock().unwrap().clear();

    assert!(!tree.update(1.0 / 60.0));
    assert!(tree.invalidation().lock().unwrap().is_empty());
}
