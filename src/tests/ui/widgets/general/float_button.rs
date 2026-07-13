use crate::tests::common::*;
use crate::ui::widgets::general::float_button::*;

#[test]
fn measure_preserves_float_button_zero_layout_footprint() {
    let measured = FloatButton::new("+").measure(Constraints::loose(Size::new(40.0, 40.0)));

    assert_eq!(measured, Size::zero());
}
