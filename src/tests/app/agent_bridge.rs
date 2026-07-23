use crate::app::agent_bridge::{
    AgentWaitCondition, AgentWaitError, AgentWaitOutcome, MAX_AGENT_WAIT_TIMEOUT,
};
use crate::app::agent_control::{AgentCommandResponse, AgentSubmitError, AgentWindowAction};
use crate::app::app_timer::AppTimerQueue;
use crate::app::main_thread_queue::MainThreadQueue;
use crate::app::session_runtime::AppRuntime;
use crate::app::window_session::WindowSession;
use crate::native::traits::event::EventLoopWaker;
use crate::tests::common::*;
use crate::ui::semantic_action::SemanticAction;
use crate::ui::semantic_snapshot::SemanticTarget;
use crate::ui::view::combinators::button;
use crate::ui::view::StyleExt;
use std::sync::Barrier;
use std::thread;

fn register_runtime_session(runtime: &AppRuntime, window_id: WindowId) {
    runtime.register_session(
        window_id,
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
}

#[test]
fn bridge_is_explicit_and_only_exposes_created_live_windows() {
    let runtime = AppRuntime::new();
    let window_id = WindowId::new(41);
    register_runtime_session(&runtime, window_id);

    assert!(runtime.agent_bridge().is_none());
    assert!(runtime.enable_agent_control());
    assert!(!runtime.enable_agent_control());
    let bridge = runtime.agent_bridge().expect("enabled process bridge");
    assert!(bridge.list_windows().unwrap().is_empty());
    assert_eq!(
        bridge.snapshot(window_id).unwrap_err(),
        AgentSubmitError::WindowNotFound,
        "a reserved or guessed session id must not be controllable"
    );

    let stale_registration = runtime
        .register_agent_window(window_id, "first".to_owned(), false, true)
        .expect("first window registration");
    assert_eq!(stale_registration.generation(), 1);

    register_runtime_session(&runtime, window_id);
    assert!(bridge.list_windows().unwrap().is_empty());
    let current_registration = runtime
        .register_agent_window(window_id, "second".to_owned(), false, true)
        .expect("replacement window registration");
    assert_eq!(current_registration.generation(), 2);

    stale_registration.publish_availability(true, false);
    let windows = bridge.list_windows().unwrap();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].title, "second");
    assert_eq!(windows[0].generation, 2);
    assert!(!windows[0].visible);
    assert!(windows[0].presentable);
}

#[test]
fn process_bridge_lists_metadata_and_routes_snapshot_and_perform() {
    let runtime = AppRuntime::new();
    assert!(runtime.enable_agent_control());
    let window_id = WindowId::new(42);
    register_runtime_session(&runtime, window_id);

    let registration = runtime
        .register_agent_window(window_id, "agent window".to_owned(), false, true)
        .expect("live window registration");
    let mut session = WindowSession::from_root_for_window(
        window_id,
        button("Run").automation_id("run"),
        Box::new(Renderer::test()),
        320,
        160,
    );
    session.set_agent_command_queue(runtime.agent_command_queue(window_id).unwrap());
    assert!(session.bind_agent_window(registration));

    let bridge = runtime.agent_bridge().expect("enabled process bridge");
    let initial = bridge.list_windows().unwrap();
    assert_eq!(initial.len(), 1);
    assert_eq!(initial[0].window_id, window_id);
    assert_eq!(initial[0].generation, 1);
    assert_eq!(initial[0].revision, 1);
    assert_eq!(initial[0].presented_revision, 0);
    assert!(!initial[0].visible);
    assert!(initial[0].presentable);

    let snapshot_ticket = bridge.snapshot(window_id).unwrap();
    {
        let parts = session.parts_mut();
        assert!(parts
            .agent_commands
            .drain_ready(parts.tree, parts.semantic_state, true,));
    }
    let snapshot = snapshot_ticket
        .recv_timeout(Duration::from_millis(20))
        .unwrap()
        .unwrap();
    assert!(matches!(
        snapshot,
        AgentCommandResponse::Snapshot(ref snapshot)
            if snapshot.window_id == window_id
                && snapshot.generation == 1
                && snapshot.revision == 1
    ));

    let perform_ticket = bridge
        .perform(
            window_id,
            1,
            Some(1),
            SemanticTarget::AutomationId("run".to_owned()),
            SemanticAction::Focus,
        )
        .unwrap();
    {
        let parts = session.parts_mut();
        assert!(parts
            .agent_commands
            .drain_ready(parts.tree, parts.semantic_state, true,));
        assert!(parts.semantic_state.refresh(parts.tree));
        assert!(parts
            .agent_commands
            .finish_or_defer(parts.semantic_state, false));
    }
    assert!(matches!(
        perform_ticket
            .recv_timeout(Duration::from_millis(20))
            .unwrap()
            .unwrap(),
        AgentCommandResponse::Performed {
            window_id: id,
            generation: 1,
            revision: 2,
            presented_revision: 0,
            settled: true,
        } if id == window_id
    ));

    let key_ticket = bridge
        .perform_window(
            window_id,
            1,
            Some(2),
            AgentWindowAction::PressKey {
                key: KeyCode::Enter,
                modifiers: KeyMod::NONE,
            },
        )
        .unwrap();
    {
        let parts = session.parts_mut();
        assert!(parts
            .agent_commands
            .drain_ready(parts.tree, parts.semantic_state, true));
        assert!(parts
            .agent_commands
            .finish_or_defer(parts.semantic_state, false));
        parts.semantic_state.publish_agent_availability(true, false);
    }
    assert!(matches!(
        key_ticket
            .recv_timeout(Duration::from_millis(20))
            .unwrap()
            .unwrap(),
        AgentCommandResponse::Performed {
            window_id: id,
            generation: 1,
            revision: 2,
            presented_revision: 0,
            settled: true,
        } if id == window_id
    ));

    let updated = bridge.list_windows().unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].revision, 2);
    assert!(updated[0].visible);
    assert!(!updated[0].presentable);

    session.try_shutdown().unwrap();
    assert!(bridge.list_windows().unwrap().is_empty());
    assert_eq!(
        bridge.snapshot(window_id).unwrap_err(),
        AgentSubmitError::WindowNotFound
    );

    runtime.shutdown_all();
    assert_eq!(
        bridge.list_windows().unwrap_err(),
        AgentSubmitError::AppClosed
    );
    assert_eq!(
        bridge.snapshot(window_id).unwrap_err(),
        AgentSubmitError::AppClosed
    );
}

#[test]
fn wait_checks_generation_and_already_satisfied_conditions_without_ui_work() {
    let runtime = AppRuntime::new();
    assert!(runtime.enable_agent_control());
    let window_id = WindowId::new(43);
    register_runtime_session(&runtime, window_id);
    let registration = runtime
        .register_agent_window(window_id, "wait".to_owned(), true, true)
        .expect("live window registration");
    let mut session = WindowSession::from_root_for_window(
        window_id,
        button("Wait").automation_id("wait"),
        Box::new(Renderer::test()),
        240,
        80,
    );
    assert!(session.bind_agent_window(registration));
    let bridge = runtime.agent_bridge().expect("enabled process bridge");

    assert!(matches!(
        bridge
            .wait(
                window_id,
                1,
                AgentWaitCondition::RevisionAfter(0),
                Duration::ZERO,
            )
            .unwrap(),
        AgentWaitOutcome::Changed(ref window) if window.revision == 1
    ));
    assert_eq!(
        bridge
            .wait(
                window_id,
                2,
                AgentWaitCondition::RevisionAfter(0),
                Duration::ZERO,
            )
            .unwrap_err(),
        AgentWaitError::StaleWindow {
            expected: 2,
            actual: 1,
        }
    );

    assert!(session.parts_mut().semantic_state.mark_presented());
    assert!(matches!(
        bridge
            .wait(
                window_id,
                1,
                AgentWaitCondition::PresentedAtLeast(1),
                Duration::ZERO,
            )
            .unwrap(),
        AgentWaitOutcome::Presented(ref window)
            if window.revision == 1 && window.presented_revision == 1
    ));
}

#[test]
fn wait_blocks_in_bridge_and_revision_or_present_notifications_do_not_wake_ui() {
    let runtime = AppRuntime::new();
    assert!(runtime.enable_agent_control());
    let window_id = WindowId::new(44);
    register_runtime_session(&runtime, window_id);
    let registration = runtime
        .register_agent_window(window_id, "wait notifications".to_owned(), true, true)
        .expect("live window registration");
    let mut session = WindowSession::from_root_for_window(
        window_id,
        button("Wait").automation_id("wait"),
        Box::new(Renderer::test()),
        240,
        80,
    );
    assert!(session.bind_agent_window(registration));
    let wake_calls = Arc::new(AtomicUsize::new(0));
    runtime.set_event_loop_waker(EventLoopWaker::new({
        let wake_calls = wake_calls.clone();
        move || {
            wake_calls.fetch_add(1, Ordering::Relaxed);
        }
    }));

    let bridge = runtime.agent_bridge().expect("enabled process bridge");
    let revision_barrier = Arc::new(Barrier::new(2));
    let revision_waiter = thread::spawn({
        let bridge = bridge.clone();
        let barrier = revision_barrier.clone();
        move || {
            barrier.wait();
            bridge.wait(
                window_id,
                1,
                AgentWaitCondition::RevisionAfter(1),
                Duration::from_secs(1),
            )
        }
    });
    revision_barrier.wait();
    {
        let parts = session.parts_mut();
        let root = parts.tree.root_id().expect("button root");
        parts.tree.set_focus(Some(root));
        assert!(parts.semantic_state.refresh(parts.tree));
    }
    assert!(matches!(
        revision_waiter.join().unwrap().unwrap(),
        AgentWaitOutcome::Changed(ref window) if window.revision == 2
    ));

    let present_barrier = Arc::new(Barrier::new(2));
    let present_waiter = thread::spawn({
        let bridge = bridge.clone();
        let barrier = present_barrier.clone();
        move || {
            barrier.wait();
            bridge.wait(
                window_id,
                1,
                AgentWaitCondition::PresentedAtLeast(2),
                Duration::from_secs(1),
            )
        }
    });
    present_barrier.wait();
    assert!(session.parts_mut().semantic_state.mark_presented());
    assert!(matches!(
        present_waiter.join().unwrap().unwrap(),
        AgentWaitOutcome::Presented(ref window)
            if window.revision == 2 && window.presented_revision == 2
    ));
    assert_eq!(wake_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn wait_has_a_hard_timeout_and_observes_window_and_app_close() {
    let runtime = AppRuntime::new();
    assert!(runtime.enable_agent_control());
    let window_id = WindowId::new(45);
    register_runtime_session(&runtime, window_id);
    let registration = runtime
        .register_agent_window(window_id, "wait close".to_owned(), true, true)
        .expect("live window registration");
    let mut session = WindowSession::from_root_for_window(
        window_id,
        button("Wait").automation_id("wait"),
        Box::new(Renderer::test()),
        240,
        80,
    );
    assert!(session.bind_agent_window(registration));
    let bridge = runtime.agent_bridge().expect("enabled process bridge");

    let timeout = bridge
        .wait(
            window_id,
            1,
            AgentWaitCondition::RevisionAfter(1),
            Duration::from_millis(5),
        )
        .unwrap_err();
    assert_eq!(timeout, AgentWaitError::Timeout);
    assert_eq!(timeout.code().as_str(), "timeout");

    let invalid_timeout = bridge
        .wait(
            window_id,
            1,
            AgentWaitCondition::RevisionAfter(1),
            MAX_AGENT_WAIT_TIMEOUT + Duration::from_millis(1),
        )
        .unwrap_err();
    assert!(matches!(
        invalid_timeout,
        AgentWaitError::InvalidTimeout { .. }
    ));
    assert_eq!(invalid_timeout.code().as_str(), "invalid_request");

    let close_barrier = Arc::new(Barrier::new(2));
    let close_waiter = thread::spawn({
        let bridge = bridge.clone();
        let barrier = close_barrier.clone();
        move || {
            barrier.wait();
            bridge.wait(
                window_id,
                1,
                AgentWaitCondition::RevisionAfter(u64::MAX),
                Duration::from_secs(1),
            )
        }
    });
    close_barrier.wait();
    session.try_shutdown().unwrap();
    assert!(matches!(
        close_waiter.join().unwrap().unwrap(),
        AgentWaitOutcome::Closed(ref window)
            if window.closed && window.revision == 2 && !window.presentable
    ));

    runtime.shutdown_all();
    assert_eq!(
        bridge
            .wait(
                window_id,
                1,
                AgentWaitCondition::RevisionAfter(0),
                Duration::ZERO,
            )
            .unwrap_err(),
        AgentWaitError::AppClosed
    );
}
