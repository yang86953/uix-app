use crate::core::{Constraints, Size};
use crate::ui::traits::WidgetAnimation;
use crate::ui::traits::WidgetLayout;
use crate::ui::widgets::input::Select;

#[test]
fn select_enter_animation_finishes_open() {
    let mut select = Select::new().options(vec!["A", "B"]);

    select.open();
    assert!(select.is_open());
    assert!(select.is_present());

    assert!(WidgetAnimation::update_animation(&mut select, 0.05));
    assert!(!WidgetAnimation::update_animation(&mut select, 1.0));
    assert!(select.is_open());
    assert!(select.is_present());
}

#[test]
fn select_exit_animation_stays_present_until_finished() {
    let mut select = Select::new().options(vec!["A", "B"]);
    select.open();
    assert!(!WidgetAnimation::update_animation(&mut select, 1.0));

    select.close();
    assert!(!select.is_open());
    assert!(select.is_present());

    assert!(WidgetAnimation::update_animation(&mut select, 0.03));
    assert!(!WidgetAnimation::update_animation(&mut select, 1.0));
    assert!(!select.is_open());
    assert!(!select.is_present());
}

#[test]
fn measure_clamps_select_size() {
    let measured = Select::new()
        .options(vec!["Alpha", "Beta"])
        .measure(Constraints::loose(Size::new(100.0, 24.0)));

    assert_eq!(measured, Size::new(100.0, 24.0));
}
