use crate::tests::common::*;
use std::collections::BTreeMap;
use std::sync::{ Arc, Mutex,
};
use crate::app::app_timer::{AppTimerQueue, TimerHandle};
use crate::app::main_thread_queue::{MainThreadContext, MainThreadQueue};
use crate::app::window_config::WindowConfig;
use crate::app::session_runtime::*;
use crate::native::traits::event::EventLoopWaker;
use crate::ui::view::combinators::label;

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
fn post_to_ui_wakes_event_loop_after_enqueue() {
    let runtime = AppRuntime::new();
    let queue = MainThreadQueue::new();
    let wake_calls = Arc::new(AtomicUsize::new(0));
    let window_id = WindowId::new(6);
    runtime.set_event_loop_waker(EventLoopWaker::new({
        let wake_calls = wake_calls.clone();
        move || {
            wake_calls.fetch_add(1, Ordering::Relaxed);
        }
    }));
    runtime.register_session(
        window_id,
        AppTimerQueue::new(),
        queue.clone(),
        Arc::new(AtomicBool::new(true)),
    );

    runtime.post_to_ui(window_id, || {});
    runtime.enqueue_with_context(window_id, |_| {});

    assert_eq!(queue.len(), 2);
    assert_eq!(wake_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn app_timer_registration_wakes_event_loop() {
    let runtime = AppRuntime::new();
    let timers = AppTimerQueue::new();
    let wake_calls = Arc::new(AtomicUsize::new(0));
    let window_id = WindowId::new(7);
    runtime.set_event_loop_waker(EventLoopWaker::new({
        let wake_calls = wake_calls.clone();
        move || {
            wake_calls.fetch_add(1, Ordering::Relaxed);
        }
    }));
    runtime.register_session(
        window_id,
        timers.clone(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );

    let _after = runtime.run_after(window_id, Duration::from_secs(1), || {});
    let _interval = runtime.run_interval(window_id, Duration::from_secs(1), || {});

    assert_eq!(timers.len(), 2);
    assert_eq!(wake_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn closed_session_post_to_ui_does_not_wake_event_loop() {
    let runtime = AppRuntime::new();
    let queue = MainThreadQueue::new();
    let wake_calls = Arc::new(AtomicUsize::new(0));
    let window_id = WindowId::new(6);
    runtime.set_event_loop_waker(EventLoopWaker::new({
        let wake_calls = wake_calls.clone();
        move || {
            wake_calls.fetch_add(1, Ordering::Relaxed);
        }
    }));
    runtime.register_session(
        window_id,
        AppTimerQueue::new(),
        queue.clone(),
        Arc::new(AtomicBool::new(true)),
    );
    runtime.close_session(window_id);

    runtime.post_to_ui(window_id, || {});

    assert_eq!(queue.len(), 0);
    assert_eq!(wake_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn closed_session_timer_registration_does_not_wake_event_loop() {
    let runtime = AppRuntime::new();
    let timers = AppTimerQueue::new();
    let wake_calls = Arc::new(AtomicUsize::new(0));
    let window_id = WindowId::new(8);
    runtime.set_event_loop_waker(EventLoopWaker::new({
        let wake_calls = wake_calls.clone();
        move || {
            wake_calls.fetch_add(1, Ordering::Relaxed);
        }
    }));
    runtime.register_session(
        window_id,
        timers.clone(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    runtime.close_session(window_id);

    let _after = runtime.run_after(window_id, Duration::from_secs(1), || {});
    let _interval = runtime.run_interval(window_id, Duration::from_secs(1), || {});

    assert_eq!(timers.len(), 0);
    assert_eq!(wake_calls.load(Ordering::Relaxed), 0);
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
fn request_open_window_wakes_event_loop() {
    let runtime = AppRuntime::new();
    let wake_calls = Arc::new(AtomicUsize::new(0));
    runtime.set_event_loop_waker(EventLoopWaker::new({
        let wake_calls = wake_calls.clone();
        move || {
            wake_calls.fetch_add(1, Ordering::Relaxed);
        }
    }));
    runtime.register_session(
        WindowId::ROOT,
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );

    let session =
        runtime.request_open_window(WindowConfig::new("Inspector", 320, 600, || label("child")));

    assert_ne!(session.window_id, WindowId::ROOT);
    assert_eq!(wake_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn request_open_window_skips_registered_platform_window_ids() {
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(7),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );

    let session =
        runtime.request_open_window(WindowConfig::new("Inspector", 320, 600, || label("child")));

    assert_eq!(session.window_id, WindowId::new(8));
}

#[test]
fn close_session_removes_pending_open_window_request() {
    let runtime = AppRuntime::new();
    let session =
        runtime.request_open_window(WindowConfig::new("Inspector", 320, 600, || label("child")));

    runtime.close_session(session.window_id);

    assert!(runtime.take_next_open_window().is_none());
}

#[test]
fn shutdown_all_cancels_pending_and_live_window_sessions() {
    let runtime = AppRuntime::new();
    let root_timers = AppTimerQueue::new();
    let root_queue = MainThreadQueue::new();
    let root_alive = Arc::new(AtomicBool::new(true));
    runtime.register_session(
        WindowId::ROOT,
        root_timers.clone(),
        root_queue.clone(),
        root_alive.clone(),
    );
    runtime.post_to_ui(WindowId::ROOT, || {});
    let _root_timer = runtime.run_after(WindowId::ROOT, Duration::from_secs(1), || {});
    let child =
        runtime.request_open_window(WindowConfig::new("Inspector", 320, 600, || label("child")));
    let _child_timer = runtime.run_after(child.window_id, Duration::from_secs(1), || {});

    runtime.shutdown_all();

    assert!(!root_alive.load(Ordering::Acquire));
    assert!(!child.alive.load(Ordering::Acquire));
    assert_eq!(root_timers.len(), 0);
    assert_eq!(root_queue.len(), 0);
    assert!(runtime
        .sessions
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_empty());
    assert!(runtime.take_next_open_window().is_none());

    let late = runtime.request_open_window(WindowConfig::new("Late", 320, 600, || label("late")));
    assert!(!late.alive.load(Ordering::Acquire));
    assert!(runtime.take_next_open_window().is_none());
}

#[test]
fn theme_changes_coalesce_to_the_latest_value_with_one_wake() {
    let runtime = AppRuntime::new();
    let wake_calls = Arc::new(AtomicUsize::new(0));
    runtime.set_event_loop_waker(EventLoopWaker::new({
        let wake_calls = wake_calls.clone();
        move || {
            wake_calls.fetch_add(1, Ordering::Relaxed);
        }
    }));

    runtime.set_theme(Theme::antd_dark());
    runtime.set_theme(Theme::antd_light());

    assert_eq!(wake_calls.load(Ordering::Relaxed), 1);
    assert!(!runtime.take_pending_theme().unwrap().is_dark());
    assert!(runtime.take_pending_theme().is_none());
}
