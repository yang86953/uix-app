use crate::tests::common::*;
use crate::component;
use crate::draw::{ Radius };
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::widgets::input::ColorPicker;

#[test]
fn color_picker_enter_animation_finishes_open() {
    let mut picker = ColorPicker::new(Color::from_rgb(255, 0, 0));

    picker.open();
    assert!(picker.is_open());
    assert!(picker.is_present());

    assert!(WidgetAnimation::update_animation(&mut picker, 0.05));
    assert!(!WidgetAnimation::update_animation(&mut picker, 1.0));
    assert!(picker.is_open());
    assert!(picker.is_present());
}

#[test]
fn color_picker_exit_animation_stays_present_until_finished() {
    let mut picker = ColorPicker::new(Color::from_rgb(255, 0, 0));
    picker.open();
    assert!(!WidgetAnimation::update_animation(&mut picker, 1.0));

    picker.close();
    assert!(!picker.is_open());
    assert!(picker.is_present());

    assert!(WidgetAnimation::update_animation(&mut picker, 0.03));
    assert!(!WidgetAnimation::update_animation(&mut picker, 1.0));
    assert!(!picker.is_open());
    assert!(!picker.is_present());
}

#[test]
fn measure_clamps_color_picker_size() {
    let measured = ColorPicker::new(Color::from_rgb(255, 0, 0))
        .measure(Constraints::loose(Size::new(20.0, 20.0)));

    assert_eq!(measured, Size::new(20.0, 20.0));
}
