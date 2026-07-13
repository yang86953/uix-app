use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::input::date_picker::*;

#[test]
fn measure_clamps_date_picker_size() {
    let measured =
        DatePicker::new("Pick date").measure(Constraints::loose(Size::new(100.0, 20.0)));

    assert_eq!(measured, Size::new(100.0, 20.0));
}
