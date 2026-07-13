use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_tag_size() {
    let measured = Tag::new("abcdef")
        .closable()
        .measure(Constraints::loose(Size::new(40.0, 16.0)));

    assert_eq!(measured, Size::new(40.0, 16.0));
}
