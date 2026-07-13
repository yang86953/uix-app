use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_segmented_size() {
    let measured = Segmented::new()
        .options(vec!["One", "Two"])
        .measure(Constraints::loose(Size::new(70.0, 24.0)));

    assert_eq!(measured, Size::new(70.0, 24.0));
}
