use super::*;
use crate::core::Rect;
use crate::ui::core::widget::WidgetCore;
use crate::ui::traits::{WidgetAnimation, WidgetCapabilities, WidgetComponent};
use crate::ui::WidgetTree;

#[test]
fn modal_advertises_animation_capability() {
    let modal = Modal::new("Dialog").show();

    assert!(modal.capabilities().contains(WidgetCapabilities::ANIMATION));
    assert!(modal.as_animation().is_some());
}

#[test]
fn modal_enter_transition_advances_and_marks_paint_dirty() {
    let mut modal = Modal::new("Dialog").show();
    let initial_opacity = modal.transition.opacity_progress;

    assert!(WidgetAnimation::update_animation(&mut modal, 0.05));
    assert!(modal.transition.opacity_progress > initial_opacity);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Modal::new("Dialog").show()));
    tree.get_mut(id)
        .expect("modal root")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.get_mut(id).expect("modal root").set_active(true);
    tree.invalidation().lock().unwrap().clear();

    assert!(tree.update(1.0 / 60.0));

    let queue = tree.invalidation().lock().unwrap();
    assert!(queue.has_paint_or_composite());
    assert!(queue.node_needs_paint(id));
}

#[test]
fn modal_close_finishes_exit_transition_before_internal_hide() {
    let mut modal = Modal::new("Dialog").show();
    assert!(!WidgetAnimation::update_animation(&mut modal, 1.0));
    assert!(modal.is_visible());

    modal.close();
    assert!(!modal.is_visible());
    assert!(modal.is_present());

    assert!(!WidgetAnimation::update_animation(&mut modal, 1.0));
    assert!(!modal.is_present());
    assert!(modal.transition_dirty);
}
