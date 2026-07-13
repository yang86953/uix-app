use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::display::result::*;

#[test]
fn measure_clamps_result_size() {
    let measured =
        Result::new(ResultType::Success).measure(Constraints::loose(Size::new(240.0, 180.0)));

    assert_eq!(measured, Size::new(240.0, 180.0));
}
