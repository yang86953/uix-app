use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_carousel_size() {
    let measured = Carousel::new().measure(Constraints::loose(Size::new(150.0, 90.0)));

    assert_eq!(measured, Size::new(150.0, 90.0));
}
