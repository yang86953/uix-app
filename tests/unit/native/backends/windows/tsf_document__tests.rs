use super::TsfEventSink;
use crate::core::{Errc, WindowId};
use crate::diagnostics::PendingFailureQueue;
use crate::platform::windowing::event::UiEvent;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::HWND;

#[test]
fn tsf_post_message_failure_is_deferred_to_owner_boundary() {
    let queue = PendingFailureQueue::new();
    let source = queue.source();
    let sink = TsfEventSink {
        events: Arc::new(Mutex::new(VecDeque::new())),
        window_id: WindowId::new(1),
        hwnd: HWND(1usize as *mut std::ffi::c_void),
        pending_failures: source.clone(),
    };

    sink.push(vec![UiEvent::text_input("ime")]);

    let queued_events = sink
        .events
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    assert_eq!(queued_events.len(), 1);
    assert_eq!(
        queued_events.front().and_then(|event| event.window_id),
        Some(WindowId::new(1))
    );
    drop(queued_events);

    let Some(error) = source.take() else {
        panic!("TSF PostMessageW failure must reach the owner source");
    };
    assert_eq!(error.code(), Errc::PlatformError);
    assert!(error.message().contains("TSF: PostMessageW"));
    assert!(source.take().is_none());
}
