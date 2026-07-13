use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::input::rate::*;

#[test]
fn measure_clamps_rate_size() {
    let measured = Rate::new()
        .count(4)
        .measure(Constraints::loose(Size::new(80.0, 18.0)));

    assert_eq!(measured, Size::new(80.0, 18.0));
}
