use super::*;
use crate::app::main_thread_queue::MainThreadContext;
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
