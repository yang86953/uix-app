use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_radio_size() {
    let measured = Radio::new()
        .options(vec!["One", "Two"])
        .measure(Constraints::loose(Size::new(70.0, 20.0)));

    assert_eq!(measured, Size::new(70.0, 20.0));
}
