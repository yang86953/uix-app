use crate::tests::common::*;
use crate::ui::widgets::feedback::{Tooltip, TooltipPlacement};

fn assert_rect_close(actual: Rect, expected: Rect) {
    assert!(
        (actual.x - expected.x).abs() < 0.001
            && (actual.y - expected.y).abs() < 0.001
            && (actual.w - expected.w).abs() < 0.001
            && (actual.h - expected.h).abs() < 0.001,
        "expected {expected:?}, got {actual:?}"
    );
}

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

#[test]
fn cjk_tooltip_uses_visible_text_width_for_overlay_and_damage() {
    let frame = Rect::new(100.0, 100.0, 80.0, 28.0);
    let mut tooltip = Tooltip::new("提示文字").placement(TooltipPlacement::Top);
    tooltip.open();
    assert!(!WidgetAnimation::update_animation(&mut tooltip, 1.0));

    let overlay = WidgetRender::overlay_entry(&tooltip, ComponentId::new(7), frame)
        .expect("open tooltip overlay");
    let bubble = Rect::new(108.0, 66.0, 64.0, 26.0);
    assert_rect_close(overlay.bounds_rect().expect("tooltip bounds"), bubble);
    assert_rect_close(
        WidgetRender::dirty_rect(&tooltip, frame),
        frame.union(&bubble),
    );
}
