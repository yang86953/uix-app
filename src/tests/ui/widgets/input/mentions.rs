use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_mentions_size() {
    let measured = Mentions::new("Mention").measure(Constraints::loose(Size::new(60.0, 20.0)));

    assert_eq!(measured, Size::new(60.0, 20.0));
}
