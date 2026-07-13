use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::display::empty::*;

#[test]
fn measure_clamps_empty_size() {
    let measured = Empty::new().measure(Constraints::loose(Size::new(100.0, 60.0)));

    assert_eq!(measured, Size::new(100.0, 60.0));
}
