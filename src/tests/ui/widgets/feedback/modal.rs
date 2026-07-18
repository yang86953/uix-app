use crate::draw::compositor::ScenePaint;
use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::feedback::modal::*;
use crate::ui::AnimationConfig;

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
    assert!(
        tree.invalidation()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .dirty_region()
            .full_frame
    );
}

#[test]
fn completed_modal_exit_removes_its_overlay_presentation() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Modal::new("Dialog").visible(true).overlay(true)));
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
    let modal = Modal::new("Dialog").visible(true);

    assert!(modal.capabilities().contains(WidgetCapabilities::ANIMATION));
    assert!(modal.as_animation().is_some());
}

#[test]
fn modal_enter_transition_advances_and_marks_paint_dirty() {
    let mut modal = Modal::new("Dialog").visible(true);
    let initial_opacity = modal.transition.opacity_progress;

    assert!(WidgetAnimation::update_animation(&mut modal, 0.05));
    assert!(modal.transition.opacity_progress > initial_opacity);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Modal::new("Dialog").visible(true)));
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
    let mut modal = Modal::new("Dialog").visible(true);
    assert!(!WidgetAnimation::update_animation(&mut modal, 1.0));
    assert!(modal.is_visible());

    modal.close();
    assert!(!modal.is_visible());
    assert!(modal.is_present());

    assert!(!WidgetAnimation::update_animation(&mut modal, 1.0));
    assert!(!modal.is_present());
    assert!(modal.transition_dirty);
}

#[test]
fn closed_modal_hides_its_retained_child_subtree() {
    use crate::ui::view::{button, ViewAdapter, ViewNode};

    let mut tree = ViewAdapter::build(ViewNode::new(
        Modal::new("Dialog").visible(true).overlay(true),
        vec![button("Cancel").into()],
    ));
    let modal = tree.root_id().expect("modal root");
    tree.get_mut(modal)
        .expect("modal node")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.get_mut(modal).expect("modal node").set_active(true);
    tree.layout();
    assert!(!tree.update(1.0));
    tree.layout();

    let child = tree
        .get(modal)
        .expect("modal node")
        .children()
        .first()
        .copied()
        .expect("modal child");
    assert!(tree.visible_rect_for(child).is_some());

    tree.get_mut(modal)
        .expect("modal node")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Modal>()
        .expect("Modal component")
        .close();
    assert!(!tree.update(1.0));

    assert!(
        tree.visible_rect_for(child).is_none(),
        "a closed Modal must not expose stale child geometry"
    );
    assert_ne!(
        tree.hit_test(Point::new(300.0, 250.0)),
        Some(child),
        "a closed Modal child must not remain hittable"
    );

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(16.0, 16.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    tree.layout();
    assert!(!tree.update(1.0));
    tree.layout();
    assert!(
        tree.visible_rect_for(child).is_some(),
        "reopening a Modal must relayout and reveal its retained child subtree"
    );
}

#[test]
fn modal_uses_custom_enter_and_leave_animation_durations() {
    let mut modal = Modal::new("Dialog")
        .enter_animation(AnimationConfig::fade_in(0.4))
        .leave_animation(AnimationConfig::fade_out(0.3))
        .visible(true);

    assert_eq!(modal.transition.scale, 1.0);
    assert!(WidgetAnimation::update_animation(&mut modal, 0.2));
    assert!(modal.transition.opacity_progress > 0.0);
    assert!(modal.transition.opacity_progress < 1.0);

    modal.close();
    assert!(WidgetAnimation::update_animation(&mut modal, 0.2));
    assert!(modal.is_present());
    assert!(!WidgetAnimation::update_animation(&mut modal, 0.1));
    assert!(!modal.is_present());
}

#[test]
fn modal_show_builds_interactive_view_and_context_close_starts_exit() {
    use crate::ui::view::{button, ViewAdapter};
    use crate::ui::widgets::Button;

    let mut tree = ViewAdapter::build(
        Modal::show(|ctx| button("关闭").on_click_fn(move || ctx.close()))
            .title("提示")
            .width(400.0)
            .height(240.0)
            .enter_animation(AnimationConfig::fade_in(0.2))
            .leave_animation(AnimationConfig::fade_out(0.15)),
    );
    let modal_id = tree.root_id().expect("modal root");
    tree.get_mut(modal_id)
        .expect("modal node")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.get_mut(modal_id).expect("modal node").set_active(true);
    tree.layout();
    assert!(!tree.update(1.0));
    tree.layout();

    let child_id = tree
        .get(modal_id)
        .expect("modal node")
        .children()
        .first()
        .copied()
        .expect("modal content child");
    let child = tree.get(child_id).expect("modal content node");
    assert!(child.component().as_any().is::<Button>());
    let child_frame = child.frame();
    assert!(child_frame.w > 0.0 && child_frame.h > 0.0);
    assert!(matches!(
        tree.get(modal_id)
            .expect("modal node")
            .component()
            .snapshot_fields(),
        SnapshotFields::Modal {
            title,
            width: 400.0,
            height: 240.0,
            footer_visible: false,
            overlay: true,
            ..
        } if title == "提示"
    ));

    let click = Point::new(
        child_frame.x + child_frame.w * 0.5,
        child_frame.y + child_frame.h * 0.5,
    );
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

    let modal = tree
        .get(modal_id)
        .expect("modal node")
        .component()
        .as_any()
        .downcast_ref::<Modal>()
        .expect("Modal component");
    assert!(!modal.is_visible());
    assert!(modal.is_present());
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .needs_full_frame());
}
