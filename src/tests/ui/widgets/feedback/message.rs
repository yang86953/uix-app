use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_preserves_message_zero_layout_footprint() {
    let measured = Message::new().measure(Constraints::loose(Size::new(200.0, 80.0)));

    assert_eq!(measured, Size::zero());
}
