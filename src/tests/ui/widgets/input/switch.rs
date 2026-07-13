use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::input::switch::*;

#[test]
fn measure_clamps_switch_size() {
    let measured = Switch::new().measure(Constraints::loose(Size::new(32.0, 20.0)));

    assert_eq!(measured, Size::new(32.0, 20.0));
}
