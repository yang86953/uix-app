use crate::tests::common::*;
use crate::ui::widgets::feedback::Tooltip;

#[test]
fn tooltip_enter_animation_finishes_visible() {
    let mut tooltip = Tooltip::new("Help");

    tooltip.open();
    assert!(tooltip.is_visible());
    assert!(tooltip.is_present());

    assert!(WidgetAnimation::update_animation(&mut tooltip, 0.05));
    assert!(!WidgetAnimation::update_animation(&mut tooltip, 1.0));
    assert!(tooltip.is_visible());
    assert!(tooltip.is_present());
}

#[test]
fn tooltip_exit_animation_stays_present_until_finished() {
    let mut tooltip = Tooltip::new("Help");
    tooltip.open();
    assert!(!WidgetAnimation::update_animation(&mut tooltip, 1.0));

    tooltip.close();
    assert!(!tooltip.is_visible());
    assert!(tooltip.is_present());

    assert!(WidgetAnimation::update_animation(&mut tooltip, 0.03));
    assert!(!WidgetAnimation::update_animation(&mut tooltip, 1.0));
    assert!(!tooltip.is_visible());
    assert!(!tooltip.is_present());
}
