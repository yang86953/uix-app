use super::*;
use crate::core::Rect;
use crate::draw::compositor::ScenePaint;
use crate::ui::core::widget::WidgetCore;
use crate::ui::core::widget::WidgetTree;
use crate::ui::traits::{WidgetAnimation, WidgetCapabilities, WidgetComponent};

#[test]
fn closed_modal_trigger_opens_via_widget_tree_pointer_down() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Modal::new("Dialog").overlay(true)));
    tree.get_mut(id)
        .expect("modal root")
        .set_frame(Rect::new(0.0, 0.0, 96.0, 32.0));
    let closed_tree_version = tree.tree_version();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(48.0, 16.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(tree
        .get(id)
        .expect("modal root")
        .component()
        .as_any()
        .downcast_ref::<Modal>()
        .expect("modal component")
        .is_present());
    assert!(tree
        .overlay_stack()
        .iter()
        .any(|entry| { entry.owner() == id && entry.kind() == crate::ui::OverlayKind::Modal }));
    assert!(ScenePaint::node_is_overlay(&tree, id));
    assert!(
        tree.tree_version() > closed_tree_version,
        "opening an overlay must rebuild the compositor layer tree"
    );
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .needs_full_frame());
}

#[test]
fn completed_modal_exit_removes_its_overlay_presentation() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Modal::new("Dialog").show().overlay(true)));
    tree.get_mut(id)
        .expect("modal root")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.get_mut(id).expect("modal root").set_active(true);
    tree.layout();
    let open_tree_version = tree.tree_version();
    assert!(tree.overlay_stack().top().is_some());

    tree.get_mut(id)
        .expect("modal root")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Modal>()
        .expect("modal component")
        .close();
    assert!(!tree.update(1.0));

    assert!(tree.overlay_stack().is_empty());
    assert!(tree.tree_version() > open_tree_version);
}

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
