use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_date_picker_size() {
    let measured =
        DatePicker::new("Pick date").measure(Constraints::loose(Size::new(100.0, 20.0)));

    assert_eq!(measured, Size::new(100.0, 20.0));
}
