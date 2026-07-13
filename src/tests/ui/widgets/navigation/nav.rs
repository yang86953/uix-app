use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_nav_item_size() {
    let active = Rc::new(Cell::new(0));
    let measured = NavItem::new("Home", 0, active)
        .width(180.0)
        .height(36.0)
        .measure(Constraints::loose(Size::new(90.0, 20.0)));

    assert_eq!(measured, Size::new(90.0, 20.0));
}
