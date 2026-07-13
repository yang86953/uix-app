use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::other::theme_toggle::*;

#[test]
fn measure_clamps_theme_toggle_size() {
    let measured = ThemeToggle::new().measure(Constraints::loose(Size::new(20.0, 20.0)));

    assert_eq!(measured, Size::new(20.0, 20.0));
}
