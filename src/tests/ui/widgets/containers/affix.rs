use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_affixed_placeholder_height() {
    let mut affix = Affix::new(12.0);
    affix.set_child_bounds(0.0, 80.0);
    affix.update_scroll(20.0);

    let measured = affix.measure(Constraints::loose(Size::new(120.0, 32.0)));

    assert_eq!(measured, Size::new(0.0, 32.0));
}
