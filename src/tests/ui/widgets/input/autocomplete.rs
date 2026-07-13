use crate::tests::common::*;
use crate::component;
use crate::draw::{ Radius };
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::widgets::input::AutoComplete;

#[test]
fn autocomplete_enter_animation_finishes_open() {
    let mut autocomplete = AutoComplete::new().options(vec!["Alpha", "Beta"]);

    autocomplete.open();
    assert!(autocomplete.is_open());
    assert!(autocomplete.is_present());

    assert!(WidgetAnimation::update_animation(&mut autocomplete, 0.05));
    assert!(!WidgetAnimation::update_animation(&mut autocomplete, 1.0));
    assert!(autocomplete.is_open());
    assert!(autocomplete.is_present());
}

#[test]
fn autocomplete_exit_animation_stays_present_until_finished() {
    let mut autocomplete = AutoComplete::new().options(vec!["Alpha", "Beta"]);
    autocomplete.open();
    assert!(!WidgetAnimation::update_animation(&mut autocomplete, 1.0));

    autocomplete.close();
    assert!(!autocomplete.is_open());
    assert!(autocomplete.is_present());

    assert!(WidgetAnimation::update_animation(&mut autocomplete, 0.03));
    assert!(!WidgetAnimation::update_animation(&mut autocomplete, 1.0));
    assert!(!autocomplete.is_open());
    assert!(!autocomplete.is_present());
}

#[test]
fn measure_clamps_autocomplete_size() {
    let measured = AutoComplete::new()
        .options(vec!["Alpha", "Beta"])
        .measure(Constraints::loose(Size::new(100.0, 20.0)));

    assert_eq!(measured, Size::new(100.0, 20.0));
}
