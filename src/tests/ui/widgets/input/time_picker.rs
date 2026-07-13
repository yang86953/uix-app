use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::input::time_picker::*;

#[test]
fn measure_clamps_time_picker_size() {
    let measured = TimePicker::new("time").measure(Constraints::loose(Size::new(90.0, 24.0)));

    assert_eq!(measured, Size::new(90.0, 24.0));
}
