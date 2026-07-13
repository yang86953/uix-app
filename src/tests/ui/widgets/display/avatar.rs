use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::display::avatar::*;

#[test]
fn measure_clamps_avatar_size() {
    let measured = Avatar::new("A")
        .size(48.0)
        .measure(Constraints::loose(Size::new(32.0, 40.0)));

    assert_eq!(measured, Size::new(32.0, 40.0));
}
