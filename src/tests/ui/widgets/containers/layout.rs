use crate::tests::common::*;
use crate::component;
use crate::ui::widgets::containers::layout::*;

#[test]
fn measure_clamps_layout_shell_parts() {
    assert_eq!(
        Layout::new().measure(Constraints::loose(Size::new(120.0, 80.0))),
        Size::new(120.0, 80.0)
    );
    assert_eq!(
        Header::new(64.0).measure(Constraints::loose(Size::new(120.0, 48.0))),
        Size::new(0.0, 48.0)
    );
    assert_eq!(
        Sider::new(200.0)
            .collapsed(true)
            .collapsed_width(64.0)
            .measure(Constraints::loose(Size::new(120.0, 80.0))),
        Size::new(64.0, 0.0)
    );
    assert_eq!(
        Content::new().measure(Constraints::new(
            Size::new(10.0, 12.0),
            Size::new(120.0, 80.0),
            None,
        )),
        Size::new(10.0, 12.0)
    );
    assert_eq!(
        Footer::new(40.0).measure(Constraints::loose(Size::new(120.0, 24.0))),
        Size::new(0.0, 24.0)
    );
}
