use super::*;
use crate::ui::traits::WidgetLayout;

#[test]
fn measure_clamps_skeleton_size() {
    let measured = Skeleton::new()
        .size(120.0, 40.0)
        .measure(Constraints::loose(Size::new(80.0, 24.0)));

    assert_eq!(measured, Size::new(80.0, 24.0));
}
