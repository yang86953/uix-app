use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_typography_text_size() {
    let measured =
        Typography::heading("abcdef", 1).measure(Constraints::loose(Size::new(60.0, 40.0)));

    assert_eq!(measured, Size::new(60.0, 40.0));
}
