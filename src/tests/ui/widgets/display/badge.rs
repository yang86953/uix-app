use crate::tests::common::*;
use crate::component;
use crate::draw::spatial::PhysicalUnit;
use crate::draw::{ Radius };
use crate::ui::widgets::display::badge::*;

#[test]
fn measure_clamps_badge_size() {
    let measured = Badge::new()
        .count(1234)
        .measure(Constraints::loose(Size::new(18.0, 12.0)));

    assert_eq!(measured, Size::new(18.0, 12.0));
}
