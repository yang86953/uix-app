use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::containers::back_top::*;

#[test]
fn measure_clamps_back_top_size() {
    let measured = BackTop::new().measure(Constraints::loose(Size::new(24.0, 32.0)));

    assert_eq!(measured, Size::new(24.0, 32.0));
}
