use crate::ui::traits::WidgetAnimation;
use crate::ui::widgets::input::{Cascader, CascaderOption};

#[test]
fn cascader_enter_animation_finishes_open() {
    let mut cascader = Cascader::new(vec![CascaderOption::new("Alpha", "alpha")], "Pick");

    cascader.open();
    assert!(cascader.is_open());
    assert!(cascader.is_present());

    assert!(WidgetAnimation::update_animation(&mut cascader, 0.05));
    assert!(!WidgetAnimation::update_animation(&mut cascader, 1.0));
    assert!(cascader.is_open());
    assert!(cascader.is_present());
}

#[test]
fn cascader_exit_animation_stays_present_until_finished() {
    let mut cascader = Cascader::new(vec![CascaderOption::new("Alpha", "alpha")], "Pick");
    cascader.open();
    assert!(!WidgetAnimation::update_animation(&mut cascader, 1.0));

    cascader.close();
    assert!(!cascader.is_open());
    assert!(cascader.is_present());

    assert!(WidgetAnimation::update_animation(&mut cascader, 0.03));
    assert!(!WidgetAnimation::update_animation(&mut cascader, 1.0));
    assert!(!cascader.is_open());
    assert!(!cascader.is_present());
}
