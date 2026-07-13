use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::input::mentions::*;

#[test]
fn measure_clamps_mentions_size() {
    let measured = Mentions::new("Mention").measure(Constraints::loose(Size::new(60.0, 20.0)));

    assert_eq!(measured, Size::new(60.0, 20.0));
}
