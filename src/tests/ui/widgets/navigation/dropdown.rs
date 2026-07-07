use crate::ui::traits::WidgetAnimation;
use crate::ui::widgets::navigation::Dropdown;

#[test]
fn dropdown_enter_animation_finishes_open() {
    let mut dropdown = Dropdown::new("Menu").items(vec!["A", "B"]);

    dropdown.open();
    assert!(dropdown.is_open());
    assert!(dropdown.is_present());

    assert!(WidgetAnimation::update_animation(&mut dropdown, 0.05));
    assert!(!WidgetAnimation::update_animation(&mut dropdown, 1.0));
    assert!(dropdown.is_open());
    assert!(dropdown.is_present());
}

#[test]
fn dropdown_exit_animation_stays_present_until_finished() {
    let mut dropdown = Dropdown::new("Menu").items(vec!["A", "B"]);
    dropdown.open();
    assert!(!WidgetAnimation::update_animation(&mut dropdown, 1.0));

    dropdown.close();
    assert!(!dropdown.is_open());
    assert!(dropdown.is_present());

    assert!(WidgetAnimation::update_animation(&mut dropdown, 0.03));
    assert!(!WidgetAnimation::update_animation(&mut dropdown, 1.0));
    assert!(!dropdown.is_open());
    assert!(!dropdown.is_present());
}
