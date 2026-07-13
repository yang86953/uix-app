use crate::tests::common::*;
use crate::component;
use crate::ui::children::WidgetChildren;
use crate::ui::widgets::display::carousel::*;

#[test]
fn measure_clamps_carousel_size() {
    let measured = Carousel::new().measure(Constraints::loose(Size::new(150.0, 90.0)));

    assert_eq!(measured, Size::new(150.0, 90.0));
}
