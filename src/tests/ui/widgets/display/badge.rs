use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_badge_size() {
    let measured = Badge::new()
        .count(1234)
        .measure(Constraints::loose(Size::new(18.0, 12.0)));

    assert_eq!(measured, Size::new(18.0, 12.0));
}
