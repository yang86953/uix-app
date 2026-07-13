use crate::tests::common::*;
use crate::component;
use crate::draw::Radius;
use crate::ui::widgets::feedback::alert::*;

#[test]
fn measure_clamps_alert_size() {
    let measured = Alert::new("info")
        .description("extra")
        .measure(Constraints::loose(Size::new(120.0, 40.0)));

    assert_eq!(measured, Size::new(120.0, 40.0));
}
