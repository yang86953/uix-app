use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_checkbox_size() {
    let measured = Checkbox::new("abcdef").measure(Constraints::loose(Size::new(40.0, 18.0)));

    assert_eq!(measured, Size::new(40.0, 18.0));
}
