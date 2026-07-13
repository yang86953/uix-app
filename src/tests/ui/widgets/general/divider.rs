use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::general::divider::*;

#[test]
fn measure_clamps_divider_text_height() {
    let measured = Divider::new()
        .with_text("Section")
        .measure(Constraints::loose(Size::new(80.0, 12.0)));

    assert_eq!(measured, Size::new(0.0, 12.0));
}
