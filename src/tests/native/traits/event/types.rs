use crate::native::traits::event::types::*;
use crate::tests::common::*;

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

#[test]
fn frame_opportunity_preserves_request_token_and_timestamps() {
    let window_id = WindowId::new(11);
    let token = FrameRequestToken::new(7, 29);
    let frame_time = Instant::now();
    let target = frame_time + Duration::from_millis(2);
    let event = UiEvent::frame_opportunity(token, frame_time, Some(target)).for_window(window_id);

    assert_eq!(event.window_id, Some(window_id));
    assert_eq!(event.type_, UiEventType::FrameOpportunity);
    assert_eq!(
        event.payload,
        UiEventPayload::FrameOpportunity(FrameOpportunityData {
            token,
            frame_time,
            target_present_time: Some(target),
        })
    );
}

#[test]
fn window_visibility_factories_have_no_payload() {
    assert_eq!(
        UiEvent::window_show(),
        UiEvent::new(UiEventType::WindowShow, UiEventPayload::None)
    );
    assert_eq!(
        UiEvent::window_hide(),
        UiEvent::new(UiEventType::WindowHide, UiEventPayload::None)
    );
    assert_eq!(
        UiEvent::window_occlusion_changed(),
        UiEvent::new(UiEventType::WindowOcclusionChanged, UiEventPayload::None)
    );
}
