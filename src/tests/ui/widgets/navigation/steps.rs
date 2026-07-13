use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::navigation::steps::*;

#[test]
fn measure_clamps_steps_size() {
    let measured = Steps::new(vec![Step::new("One"), Step::new("Two")])
        .measure(Constraints::loose(Size::new(120.0, 40.0)));

    assert_eq!(measured, Size::new(120.0, 40.0));
}
