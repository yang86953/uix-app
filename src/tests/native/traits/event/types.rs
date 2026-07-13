use crate::tests::common::*;
use crate::native::traits::event::types::*;

#[test]
fn for_window_tags_event_without_changing_payload() {
    let window_id = WindowId::new(7);
    let event = UiEvent::resize(320, 200).for_window(window_id);

    assert_eq!(event.window_id, Some(window_id));
    assert_eq!(event.type_, UiEventType::WindowResize);
    assert_eq!(
        event.payload,
        UiEventPayload::Resize(ResizeData {
            width: 320,
            height: 200,
        })
    );
}
