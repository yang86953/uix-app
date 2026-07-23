use crate::app::app_handle::*;
use crate::app::app_timer::AppTimerQueue;
use crate::app::main_thread_queue::MainThreadContext;
use crate::app::main_thread_queue::MainThreadQueue;
use crate::app::session_runtime::AppRuntime;
use crate::app::window_config::WindowConfig;
use crate::app::Container as DiContainer;
#[cfg(feature = "test-harness")]
use crate::draw::renderer::test_harness::GraphicsFaultSignal;
use crate::native::traits::event::EventLoopWaker;
use crate::tests::common::*;
use crate::ui::view::combinators::label;
use crate::ui::view::{View, ViewAdapter};
use crate::ui::widgets::feedback::notification::Notification;
use crate::ui::widgets::Label;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

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
        DiContainer::new(),
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
        DiContainer::new(),
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
fn app_handle_set_theme_updates_the_app_wide_runtime() {
    let runtime = AppRuntime::new();
    let alive = Arc::new(AtomicBool::new(true));
    runtime.register_session(
        WindowId::ROOT,
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        alive.clone(),
    );
    let handle = AppHandle::new(
        WindowId::ROOT,
        AppState::new(),
        runtime.clone(),
        DiContainer::new(),
        alive,
    );

    handle.set_theme(Theme::antd_dark()).unwrap();

    assert!(runtime.take_pending_theme().unwrap().is_dark());
}

#[test]
fn app_handle_set_theme_rejects_closed_handles() {
    let runtime = AppRuntime::new();
    let alive = Arc::new(AtomicBool::new(true));
    runtime.register_session(
        WindowId::ROOT,
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        alive.clone(),
    );
    let handle = AppHandle::new(
        WindowId::ROOT,
        AppState::new(),
        runtime.clone(),
        DiContainer::new(),
        alive,
    );
    handle.mark_closed();

    let error = handle.set_theme(Theme::antd_dark()).unwrap_err();

    assert_eq!(error.code(), Errc::InvalidState);
    assert!(runtime.take_pending_theme().is_none());
}

#[cfg(feature = "test-harness")]
#[test]
fn app_handle_arms_one_device_lost_fault_for_its_window() {
    let runtime = AppRuntime::new();
    let alive = Arc::new(AtomicBool::new(true));
    let graphics_faults = GraphicsFaultSignal::default();
    graphics_faults.attach_recovery_driver();
    runtime.register_session_with_graphics_faults(
        WindowId::ROOT,
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        alive.clone(),
        graphics_faults.clone(),
    );
    let handle = AppHandle::new(
        WindowId::ROOT,
        AppState::new(),
        runtime,
        DiContainer::new(),
        alive,
    );

    handle
        .inject_graphics_device_lost_for_test()
        .expect("arm first device-lost fault");
    let duplicate = handle
        .inject_graphics_device_lost_for_test()
        .expect_err("reject duplicate pending fault");

    assert_eq!(duplicate.code(), Errc::InvalidState);
    assert!(graphics_faults.take_device_lost());
    assert!(!graphics_faults.take_device_lost());
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
fn app_handle_notify_error_updates_default_overlay_queue() {
    let timers = AppTimerQueue::new();
    let queue = MainThreadQueue::new();
    let alive = Arc::new(AtomicBool::new(true));
    let runtime = AppRuntime::new();
    runtime.register_session(WindowId::ROOT, timers, queue.clone(), alive.clone());
    let notifications = AppNotificationState::new();
    let mut container = DiContainer::new();
    container.singleton(notifications.clone());
    let handle = AppHandle::new(WindowId::ROOT, AppState::new(), runtime, container, alive);

    let id = handle.notify_error(&Error::warn(Errc::InvalidState, "cache is stale"));
    let fatal = handle.notify_error(&Error::fatal(Errc::InvalidState, "must abort"));

    assert_eq!(id, Some(0));
    assert_eq!(fatal, None);
    assert_eq!(notifications.handle(WindowId::ROOT).items().len(), 1);
    assert_eq!(
        notifications.handle(WindowId::ROOT).items()[0].title,
        "Warning"
    );
    assert!(notifications.handle(WindowId::ROOT).items()[0]
        .description
        .contains("cache is stale"));
    assert_eq!(queue.len(), 1);
}

#[test]
fn app_overlay_root_keeps_app_root_and_mounts_notification() {
    let notifications = AppNotificationState::new();
    notifications.notify_error(WindowId::ROOT, &Error::new(Errc::IoError, "save failed"));

    let tree = ViewAdapter::build_nodes(wrap_root_with_notification_overlay(
        label("root"),
        notifications,
        WindowId::ROOT,
    ));

    let labels = tree.find_all_by_type::<Label>();
    let notifications = tree.find_all_by_type::<Notification>();
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].1.text(), "root");
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].1.items().len(), 1);
}

#[test]
fn mounted_app_overlay_observes_later_notifications() {
    let notifications = AppNotificationState::new();
    let tree = ViewAdapter::build_nodes(wrap_root_with_notification_overlay(
        label("root"),
        notifications.clone(),
        WindowId::ROOT,
    ));
    assert!(tree.find_all_by_type::<Notification>()[0]
        .1
        .items()
        .is_empty());

    notifications.notify_error(
        WindowId::ROOT,
        &Error::warn(Errc::InvalidState, "changed after mount"),
    );

    let mounted = tree.find_all_by_type::<Notification>();
    assert_eq!(mounted[0].1.items().len(), 1);
    assert!(mounted[0].1.items()[0]
        .description
        .contains("changed after mount"));
}

#[test]
fn app_notification_queues_are_isolated_per_window() {
    let notifications = AppNotificationState::new();
    let child = WindowId::new(17);

    notifications.notify_error(
        WindowId::ROOT,
        &Error::warn(Errc::InvalidState, "root only"),
    );
    notifications.notify_error(child, &Error::new(Errc::IoError, "child only"));

    let root_items = notifications.handle(WindowId::ROOT).items();
    let child_items = notifications.handle(child).items();
    assert_eq!(root_items.len(), 1);
    assert!(root_items[0].description.contains("root only"));
    assert_eq!(child_items.len(), 1);
    assert!(child_items[0].description.contains("child only"));
}

#[test]
fn app_overlay_root_passes_clicks_to_app_when_notifications_empty() {
    use crate::ui::core::widget::WidgetCore;
    use crate::ui::view::button;

    let clicks = Rc::new(Cell::new(0));
    let clicks_for_handler = clicks.clone();
    let mut tree = ViewAdapter::build_nodes(wrap_root_with_notification_overlay(
        button("Navigate")
            .on_click_fn(move || {
                clicks_for_handler.set(clicks_for_handler.get() + 1);
            })
            .build(),
        AppNotificationState::new(),
        WindowId::ROOT,
    ));
    if let Some(root) = tree.root_mut() {
        root.set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    }
    tree.layout();

    let pos = Point::new(40.0, 40.0);
    let _ = tree.dispatch_event(&SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(clicks.get(), 1);
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
        DiContainer::new(),
        child_alive,
    );

    let _timer = handle.run_after(Duration::from_secs(1), || {});

    assert_eq!(root_timers.len(), 0);
    assert_eq!(child_timers.len(), 1);
}

#[test]
fn app_handle_timer_registration_wakes_event_loop() {
    let timers = AppTimerQueue::new();
    let queue = MainThreadQueue::new();
    let alive = Arc::new(AtomicBool::new(true));
    let runtime = AppRuntime::new();
    let wake_calls = Arc::new(AtomicUsize::new(0));
    runtime.set_event_loop_waker(EventLoopWaker::new({
        let wake_calls = wake_calls.clone();
        move || {
            wake_calls.fetch_add(1, Ordering::Relaxed);
        }
    }));
    runtime.register_session(WindowId::ROOT, timers.clone(), queue, alive.clone());
    let handle = AppHandle::new(
        WindowId::ROOT,
        AppState::new(),
        runtime,
        DiContainer::new(),
        alive,
    );

    let _after = handle.run_after(Duration::from_secs(1), || {});
    let _interval = handle.run_interval(Duration::from_secs(1), || {});

    assert_eq!(timers.len(), 2);
    assert_eq!(wake_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn app_handle_open_window_returns_child_handle_and_queues_request() {
    let timers = AppTimerQueue::new();
    let queue = MainThreadQueue::new();
    let alive = Arc::new(AtomicBool::new(true));
    let runtime = AppRuntime::new();
    let wake_calls = Arc::new(AtomicUsize::new(0));
    runtime.set_event_loop_waker(EventLoopWaker::new({
        let wake_calls = wake_calls.clone();
        move || {
            wake_calls.fetch_add(1, Ordering::Relaxed);
        }
    }));
    runtime.register_session(WindowId::ROOT, timers, queue, alive.clone());
    let handle = AppHandle::new(
        WindowId::ROOT,
        AppState::new(),
        runtime.clone(),
        DiContainer::new(),
        alive,
    );

    let child = handle
        .open_window(WindowConfig::new("Inspector", 320, 600, || label("child")))
        .unwrap();
    let request = runtime.take_next_open_window().unwrap();

    assert_ne!(child.window_id(), handle.window_id());
    assert_eq!(request.window_id, child.window_id());
    assert_eq!(request.config.title, "Inspector");
    assert_eq!(request.config.width, 320);
    assert_eq!(request.config.height, 600);
    assert_eq!(wake_calls.load(Ordering::Relaxed), 1);

    let tree = ViewAdapter::build_nodes((request.config.root)());
    let label = tree
        .root()
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .unwrap();
    assert_eq!(label.text(), "child");
}

#[test]
fn app_handle_open_window_rejects_closed_handle() {
    let timers = AppTimerQueue::new();
    let queue = MainThreadQueue::new();
    let alive = Arc::new(AtomicBool::new(true));
    let handle = new_test_handle(timers, queue, alive);

    handle.mark_closed();
    let result = handle.open_window(WindowConfig::new("ignored", 1, 1, || label("ignored")));

    assert!(result.is_err());
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
