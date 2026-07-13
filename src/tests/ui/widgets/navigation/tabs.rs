use crate::tests::common::*;
use crate::ui::widgets::navigation::tabs::*;

#[test]
fn measure_clamps_tabs_size() {
    let measured = Tabs::new()
        .tab("One", "one")
        .measure(Constraints::loose(Size::new(120.0, 80.0)));

    assert_eq!(measured, Size::new(120.0, 80.0));
}
