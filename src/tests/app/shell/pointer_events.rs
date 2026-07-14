use crate::app::map_ui_event;
use crate::core::Point;
use crate::native::traits::event::UiEvent;
use crate::native::traits::input::MouseButton;
use crate::ui::SystemEvent;

#[test]
fn map_pointer_double_click_preserves_button_and_position() {
    let event = UiEvent::pointer_double_click(Point::new(12.5, 8.25), MouseButton::Right);

    assert!(matches!(
        map_ui_event(&event),
        Some(SystemEvent::PointerDoubleClick { pos, button: MouseButton::Right, .. })
            if pos == Point::new(12.5, 8.25)
    ));
}
