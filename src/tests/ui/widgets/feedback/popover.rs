use crate::ui::traits::WidgetAnimation;
use crate::ui::widgets::feedback::Popover;

#[test]
fn popover_enter_animation_finishes_visible() {
    let mut popover = Popover::new("Details");

    popover.open();
    assert!(popover.is_visible());
    assert!(popover.is_present());

    assert!(WidgetAnimation::update_animation(&mut popover, 0.05));
    assert!(!WidgetAnimation::update_animation(&mut popover, 1.0));
    assert!(popover.is_visible());
    assert!(popover.is_present());
}

#[test]
fn popover_exit_animation_stays_present_until_finished() {
    let mut popover = Popover::new("Details");
    popover.open();
    assert!(!WidgetAnimation::update_animation(&mut popover, 1.0));

    popover.close();
    assert!(!popover.is_visible());
    assert!(popover.is_present());

    assert!(WidgetAnimation::update_animation(&mut popover, 0.03));
    assert!(!WidgetAnimation::update_animation(&mut popover, 1.0));
    assert!(!popover.is_visible());
    assert!(!popover.is_present());
}
