use super::Window;
use crate::native::test_harness::FakePlatform;
use crate::native::traits::event::UiEvent;
use std::cell::Cell;

#[test]
fn window_run_does_not_call_frame_fn_without_event() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    let mut window = Window::new(Box::new(platform));
    let calls = Cell::new(0);

    let status = window.run(|_| {
        calls.set(calls.get() + 1);
        true
    });

    assert_eq!(status, 0);
    assert_eq!(calls.get(), 0);
}

#[test]
fn window_run_calls_frame_fn_after_event() {
    let mut platform = FakePlatform::new();
    platform.event_source.inject(UiEvent::close());
    let mut window = Window::new(Box::new(platform));
    let calls = Cell::new(0);

    let status = window.run(|_| {
        calls.set(calls.get() + 1);
        false
    });

    assert_eq!(status, 0);
    assert_eq!(calls.get(), 1);
}
