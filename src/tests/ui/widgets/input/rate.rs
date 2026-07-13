use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_rate_size() {
    let measured = Rate::new()
        .count(4)
        .measure(Constraints::loose(Size::new(80.0, 18.0)));

    assert_eq!(measured, Size::new(80.0, 18.0));
}
