use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_slider_size() {
    let measured = Slider::new().measure(Constraints::loose(Size::new(120.0, 16.0)));

    assert_eq!(measured, Size::new(120.0, 16.0));
}
