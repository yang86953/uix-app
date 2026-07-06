use super::*;
use crate::app::main_thread_queue::MainThreadContext;
use crate::app::WindowConfig;
use crate::ui::view::combinators::label;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

#[test]
fn close_session_drops_queued_work_and_timers() {
    let runtime = AppRuntime::new();
    let queue = MainThreadQueue::new();
    let timers = AppTimerQueue::new();
    let alive = Arc::new(AtomicBool::new(true));
    let window_id = WindowId::new(3);
    runtime.register_session(window_id, timers.clone(), queue.clone(), alive.clone());

    runtime.post_to_ui(window_id, || {});
    let _timer = runtime.run_after(window_id, Duration::from_secs(1), || {});
    runtime.close_session(window_id);

    assert!(!alive.load(Ordering::Acquire));
    assert_eq!(queue.len(), 0);
    assert_eq!(timers.len(), 0);
}

#[test]
fn closed_session_drops_late_post_to_ui() {
    let runtime = AppRuntime::new();
    let queue = MainThreadQueue::new();
    let window_id = WindowId::new(4);
    runtime.register_session(
        window_id,
        AppTimerQueue::new(),
        queue.clone(),
        Arc::new(AtomicBool::new(true)),
    );
    runtime.close_session(window_id);

    runtime.post_to_ui(window_id, || {});

    assert_eq!(queue.len(), 0);
}

#[test]
fn routed_post_to_ui_drains_target_queue() {
    let runtime = AppRuntime::new();
    let queue = MainThreadQueue::new();
    let window_id = WindowId::new(5);
    let ran = Arc::new(AtomicUsize::new(0));
    runtime.register_session(
        window_id,
        AppTimerQueue::new(),
        queue.clone(),
        Arc::new(AtomicBool::new(true)),
    );

    runtime.post_to_ui(window_id, {
        let ran = ran.clone();
        move || {
            ran.fetch_add(1, Ordering::Relaxed);
        }
    });
    let mut pending_root = None;
    let mut reconcile_pending = false;
    let mut context = MainThreadContext::new(&mut pending_root, &mut reconcile_pending);
    assert!(queue.drain(&mut context));

    assert_eq!(ran.load(Ordering::Relaxed), 1);
}

#[test]
fn request_open_window_reserves_independent_runtime_session() {
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::ROOT,
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );

    let session =
        runtime.request_open_window(WindowConfig::new("Inspector", 320, 600, || label("child")));
    runtime.post_to_ui(session.window_id, || {});
    let _timer = runtime.run_after(session.window_id, Duration::from_secs(1), || {});
    let request = runtime.take_next_open_window().unwrap();

    assert_eq!(request.window_id, session.window_id);
    assert_eq!(request.config.title, "Inspector");
    assert_eq!(request.main_thread_queue.len(), 1);
    assert_eq!(request.app_timers.len(), 1);
    assert!(request.alive.load(Ordering::Acquire));
}

#[test]
fn close_session_removes_pending_open_window_request() {
    let runtime = AppRuntime::new();
    let session =
        runtime.request_open_window(WindowConfig::new("Inspector", 320, 600, || label("child")));

    runtime.close_session(session.window_id);

    assert!(runtime.take_next_open_window().is_none());
}
