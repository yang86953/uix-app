use crate::tests::common::*;
use crate::component;
use crate::draw::Radius;
use crate::ui::widgets::display::tag::*;

#[test]
fn measure_clamps_tag_size() {
    let measured = Tag::new("abcdef")
        .closable()
        .measure(Constraints::loose(Size::new(40.0, 16.0)));

    assert_eq!(measured, Size::new(40.0, 16.0));
}
