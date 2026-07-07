use crate::ui::traits::WidgetAnimation;
use crate::ui::widgets::feedback::Popconfirm;

#[test]
fn popconfirm_enter_animation_finishes_visible() {
    let mut popconfirm = Popconfirm::new();

    popconfirm.open();
    assert!(popconfirm.is_visible());
    assert!(popconfirm.is_present());

    assert!(WidgetAnimation::update_animation(&mut popconfirm, 0.05));
    assert!(!WidgetAnimation::update_animation(&mut popconfirm, 1.0));
    assert!(popconfirm.is_visible());
    assert!(popconfirm.is_present());
}

#[test]
fn popconfirm_exit_animation_stays_present_until_finished() {
    let mut popconfirm = Popconfirm::new();
    popconfirm.open();
    assert!(!WidgetAnimation::update_animation(&mut popconfirm, 1.0));

    popconfirm.close();
    assert!(!popconfirm.is_visible());
    assert!(popconfirm.is_present());

    assert!(WidgetAnimation::update_animation(&mut popconfirm, 0.03));
    assert!(!WidgetAnimation::update_animation(&mut popconfirm, 1.0));
    assert!(!popconfirm.is_visible());
    assert!(!popconfirm.is_present());
}
