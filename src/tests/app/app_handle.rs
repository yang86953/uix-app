use super::*;
use crate::app::app_timer::AppTimerQueue;
use crate::app::main_thread_queue::MainThreadContext;
use crate::app::main_thread_queue::MainThreadQueue;
use crate::app::session_runtime::AppRuntime;
use crate::app::Container;
use crate::ui::view::combinators::label;
use crate::ui::view::ViewAdapter;
use crate::ui::widgets::Label;
use crate::ui::AppState;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

fn drain_with_empty_context(
    queue: &MainThreadQueue,
) -> (bool, Option<crate::ui::view::ViewNode>, bool) {
    let mut pending_root = None;
    let mut reconcile_pending = false;
    let mut context = MainThreadContext::new(&mut pending_root, &mut reconcile_pending);
    let ran = queue.drain(&mut context);
    (ran, pending_root, reconcile_pending)
}

fn new_test_handle(
    timers: AppTimerQueue,
    queue: MainThreadQueue,
    alive: Arc<AtomicBool>,
) -> AppHandle {
    let runtime = AppRuntime::new();
    runtime.register_session(WindowId::root(), timers, queue, alive.clone());
    AppHandle::new(
        WindowId::root(),
        AppState::new(),
        runtime,
        Container::new(),
        alive,
    )
}

#[test]
fn app_handle_post_to_ui_enqueues_while_alive() {
    let timers = AppTimerQueue::new();
    let queue = MainThreadQueue::new();
    let alive = Arc::new(AtomicBool::new(true));
    let handle = new_test_handle(timers, queue.clone(), alive);

    handle.post_to_ui(|| {});

    assert_eq!(queue.len(), 1);
}

#[test]
fn app_handle_post_to_ui_routes_by_window_id() {
    let root_queue = MainThreadQueue::new();
    let child_queue = MainThreadQueue::new();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::ROOT,
        AppTimerQueue::new(),
        root_queue.clone(),
        Arc::new(AtomicBool::new(true)),
    );
    let child_alive = Arc::new(AtomicBool::new(true));
    let child_id = WindowId::new(9);
    runtime.register_session(
        child_id,
        AppTimerQueue::new(),
        child_queue.clone(),
        child_alive.clone(),
    );
    let handle = AppHandle::new(
        child_id,
        AppState::new(),
        runtime,
        Container::new(),
        child_alive,
    );

    handle.post_to_ui(|| {});

    assert_eq!(root_queue.len(), 0);
    assert_eq!(child_queue.len(), 1);
}

#[test]
fn app_handle_post_to_ui_drops_after_close() {
    let timers = AppTimerQueue::new();
    let queue = MainThreadQueue::new();
    let alive = Arc::new(AtomicBool::new(true));
    let handle = new_test_handle(timers, queue.clone(), alive);

    handle.mark_closed();
    handle.post_to_ui(|| {});

    assert_eq!(queue.len(), 0);
}

#[test]
fn app_handle_post_to_ui_can_be_sent_from_background_thread() {
    let timers = AppTimerQueue::new();
    let queue = MainThreadQueue::new();
    let alive = Arc::new(AtomicBool::new(true));
    let handle = new_test_handle(timers, queue.clone(), alive);
    let ran = Arc::new(AtomicUsize::new(0));

    let thread = std::thread::spawn({
        let handle = handle.clone();
        let ran = ran.clone();
        move || {
            handle.post_to_ui(move || {
                ran.fetch_add(1, Ordering::Relaxed);
            });
        }
    });
    thread.join().unwrap();

    assert_eq!(queue.len(), 1);
    assert!(drain_with_empty_context(&queue).0);
    assert_eq!(ran.load(Ordering::Relaxed), 1);
}

#[test]
fn app_handle_update_view_writes_pending_root_while_alive() {
    let timers = AppTimerQueue::new();
    let queue = MainThreadQueue::new();
    let alive = Arc::new(AtomicBool::new(true));
    let handle = new_test_handle(timers, queue.clone(), alive);

    handle.update_view(|| label("updated"));
    let (ran, pending_root, reconcile_pending) = drain_with_empty_context(&queue);
    let tree = ViewAdapter::build_nodes(pending_root.unwrap());
    let label = tree
        .root()
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .unwrap();

    assert!(ran);
    assert!(reconcile_pending);
    assert_eq!(label.text(), "updated");
}

#[test]
fn app_handle_update_view_drops_after_close() {
    let timers = AppTimerQueue::new();
    let queue = MainThreadQueue::new();
    let alive = Arc::new(AtomicBool::new(true));
    let handle = new_test_handle(timers, queue.clone(), alive);

    handle.mark_closed();
    handle.update_view(|| label("ignored"));

    assert_eq!(queue.len(), 0);
}

#[test]
fn app_handle_run_after_registers_timer_while_alive() {
    let timers = AppTimerQueue::new();
    let queue = MainThreadQueue::new();
    let alive = Arc::new(AtomicBool::new(true));
    let handle = new_test_handle(timers.clone(), queue, alive);

    let _timer = handle.run_after(Duration::from_secs(1), || {});

    assert_eq!(timers.len(), 1);
}

#[test]
fn app_handle_run_after_routes_by_window_id() {
    let root_timers = AppTimerQueue::new();
    let child_timers = AppTimerQueue::new();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::ROOT,
        root_timers.clone(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    let child_alive = Arc::new(AtomicBool::new(true));
    let child_id = WindowId::new(11);
    runtime.register_session(
        child_id,
        child_timers.clone(),
        MainThreadQueue::new(),
        child_alive.clone(),
    );
    let handle = AppHandle::new(
        child_id,
        AppState::new(),
        runtime,
        Container::new(),
        child_alive,
    );

    let _timer = handle.run_after(Duration::from_secs(1), || {});

    assert_eq!(root_timers.len(), 0);
    assert_eq!(child_timers.len(), 1);
}

#[test]
fn app_handle_run_after_returns_inert_timer_after_close() {
    let timers = AppTimerQueue::new();
    let queue = MainThreadQueue::new();
    let alive = Arc::new(AtomicBool::new(false));
    let handle = new_test_handle(timers.clone(), queue, alive);

    let timer = handle.run_after(Duration::from_secs(1), || {});
    timer.cancel();

    assert_eq!(timers.len(), 0);
}
