use crate::tests::common::*;
use crate::component;
use crate::draw::Radius;
use crate::ui::widgets::input::segmented::*;

#[test]
fn measure_clamps_segmented_size() {
    let measured = Segmented::new()
        .options(vec!["One", "Two"])
        .measure(Constraints::loose(Size::new(70.0, 24.0)));

    assert_eq!(measured, Size::new(70.0, 24.0));
}
