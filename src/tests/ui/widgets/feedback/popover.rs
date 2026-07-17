use crate::tests::common::*;
use crate::ui::traits::{EventHandler, WidgetAnimation, WidgetComponent, WidgetRender};
use crate::ui::widgets::{Popover, PopoverPlacement};
use crate::ui::{AnimationConfig, Placement};

#[test]
fn popover_is_focusable_and_keyboard_toggles_click_trigger() {
    let mut popover = Popover::new("Details");

    assert_eq!(WidgetComponent::tab_index(&popover), 1);
    assert_eq!(
        popover.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    assert_eq!(
        popover.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(popover.is_visible());
    assert_eq!(
        popover.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Escape,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!popover.is_visible());
    assert!(popover.is_present(), "exit animation remains present");
}

#[test]
fn popover_uses_custom_enter_and_leave_durations() {
    let mut popover = Popover::new("details")
        .enter_animation(AnimationConfig::fade_in(0.4))
        .leave_animation(AnimationConfig::fade_out(0.3));

    popover.open();
    assert!(WidgetAnimation::update_animation(&mut popover, 0.2));
    assert!(!WidgetAnimation::update_animation(&mut popover, 0.2));

    popover.close();
    assert!(WidgetAnimation::update_animation(&mut popover, 0.2));
    assert!(popover.is_present());
    assert!(!WidgetAnimation::update_animation(&mut popover, 0.1));
    assert!(!popover.is_present());
}

#[test]
fn slide_animation_dirty_rect_covers_the_full_motion_sweep() {
    let frame = Rect::new(300.0, 200.0, 80.0, 28.0);
    let mut popover = Popover::new("details")
        .placement(PopoverPlacement::Top)
        .enter_animation(AnimationConfig::slide_in(Placement::Right, 0.2));
    popover.open();

    let dirty = WidgetRender::dirty_rect(&popover, frame);

    assert_eq!(dirty.x, 300.0);
    assert!((dirty.x + dirty.w - 544.0).abs() < 1e-4);
    assert!(dirty.contains(Point::new(543.0, 150.0)));
}

#[test]
fn popover_overlay_is_removed_only_after_custom_leave_finishes() {
    let mut popover = Popover::new("details").leave_animation(AnimationConfig::fade_out(0.3));
    popover.open();
    assert!(!WidgetAnimation::update_animation(&mut popover, 1.0));

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(popover));
    tree.get_mut(id)
        .expect("popover root")
        .set_frame(Rect::new(300.0, 200.0, 80.0, 28.0));
    tree.get_mut(id).expect("popover root").set_active(true);
    tree.rebuild_widget_overlays();
    assert_eq!(tree.overlay_stack().len(), 1);

    tree.get_mut(id)
        .expect("popover root")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Popover>()
        .expect("popover component")
        .close();

    assert!(tree.update(0.2));
    assert_eq!(tree.overlay_stack().len(), 1);
    assert!(!tree.update(0.1));
    assert!(tree.overlay_stack().is_empty());
}

#[test]
fn animation_discovery_registers_overlay_before_enter_finishes() {
    let popover = Popover::new("details").enter_animation(AnimationConfig::fade_in(0.3));
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(popover));
    tree.get_mut(id)
        .expect("popover root")
        .set_frame(Rect::new(300.0, 200.0, 80.0, 28.0));
    tree.get_mut(id).expect("popover root").set_active(true);
    tree.rebuild_widget_overlays();
    assert!(tree.overlay_stack().is_empty());

    tree.get_mut(id)
        .expect("popover root")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Popover>()
        .expect("popover component")
        .open();

    assert!(tree.update(0.1));
    assert_eq!(tree.overlay_stack().len(), 1);
}
