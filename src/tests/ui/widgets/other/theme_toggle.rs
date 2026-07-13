use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_theme_toggle_size() {
    let measured = ThemeToggle::new().measure(Constraints::loose(Size::new(20.0, 20.0)));

    assert_eq!(measured, Size::new(20.0, 20.0));
}
