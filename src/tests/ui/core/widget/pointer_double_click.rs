use crate::core::{Point, Rect};
use crate::native::traits::input::{KeyMod, MouseButton};
use crate::ui::core::widget::WidgetCore;
use crate::ui::view::{button, ViewAdapter};
use crate::ui::widgets::Button;
use crate::ui::{EventResult, SystemEvent};

#[test]
fn unhandled_double_click_falls_back_to_second_pointer_down() {
    let mut tree = ViewAdapter::build(button("target"));
    tree.root_mut()
        .expect("button root")
        .set_frame(Rect::new(0.0, 0.0, 80.0, 32.0));
    tree.layout();

    let result = tree.dispatch_event(&SystemEvent::PointerDoubleClick {
        pos: Point::new(16.0, 10.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(result, EventResult::Handled);
    let button = tree
        .root()
        .and_then(|node| node.component().as_any().downcast_ref::<Button>())
        .expect("button component");
    assert!(button.pressed);
}
