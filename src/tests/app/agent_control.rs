use crate::app::agent_control::*;
use crate::app::app_timer::AppTimerQueue;
use crate::app::event_loop::event_loop::run_window_session_loop;
use crate::app::main_thread_queue::MainThreadQueue;
use crate::app::map_ui_event;
use crate::app::session_runtime::AppRuntime;
use crate::app::window_semantics::{WindowSemanticSnapshot, WindowSemanticState};
use crate::app::window_session::{WindowLoopState, WindowSession};
use crate::native::test_harness::{FakePlatform, FakeWindow};
use crate::native::traits::event::EventLoopWaker;
use crate::tests::common::*;
use crate::ui::semantic_action::SemanticAction;
use crate::ui::semantic_snapshot::SemanticTarget;
use crate::ui::view::combinators::button;
use crate::ui::view::{StyleExt, ViewAdapter};

fn snapshot_request() -> AgentCommandRequest {
    AgentCommandRequest::Snapshot
}

fn perform_request(
    generation: u64,
    expected_revision: Option<u64>,
    action: SemanticAction,
) -> AgentCommandRequest {
    AgentCommandRequest::Perform {
        generation,
        expected_revision,
        target: SemanticTarget::AutomationId("run".to_owned()),
        action,
    }
}

fn window_action_request(
    generation: u64,
    expected_revision: Option<u64>,
    action: AgentWindowAction,
) -> AgentCommandRequest {
    AgentCommandRequest::PerformWindow {
        generation,
        expected_revision,
        action,
    }
}

fn laid_out_button() -> WidgetTree {
    let mut tree = ViewAdapter::build_nodes(button("Run").automation_id("run"));
    tree.root_mut()
        .expect("button root")
        .set_frame(Rect::new(0.0, 0.0, 160.0, 40.0));
    tree.layout();
    tree
}

#[test]
fn command_queue_is_bounded_and_wakes_only_on_empty_to_nonempty() {
    let queue = AgentCommandQueue::with_capacity(2);

    let (first, first_wake) = queue.submit(snapshot_request()).unwrap();
    let (second, second_wake) = queue.submit(snapshot_request()).unwrap();

    assert!(first_wake);
    assert!(!second_wake);
    assert_eq!(queue.len(), 2);
    assert_eq!(
        queue.submit(snapshot_request()).unwrap_err(),
        AgentSubmitError::QueueFull
    );

    queue.close();
    assert_eq!(
        first.recv_timeout(Duration::from_millis(20)).unwrap(),
        Err(AgentCommandError::AppClosed)
    );
    assert_eq!(
        second.recv_timeout(Duration::from_millis(20)).unwrap(),
        Err(AgentCommandError::AppClosed)
    );
    assert_eq!(
        queue.submit(snapshot_request()).unwrap_err(),
        AgentSubmitError::AppClosed
    );
}

#[test]
fn runtime_routes_commands_per_window_and_coalesces_each_target_wake() {
    let runtime = AppRuntime::new();
    let first_window = WindowId::new(21);
    let second_window = WindowId::new(22);
    for window_id in [first_window, second_window] {
        runtime.register_session(
            window_id,
            AppTimerQueue::new(),
            MainThreadQueue::new(),
            Arc::new(AtomicBool::new(true)),
        );
    }
    let wake_calls = Arc::new(AtomicUsize::new(0));
    runtime.set_event_loop_waker(EventLoopWaker::new({
        let wake_calls = wake_calls.clone();
        move || {
            wake_calls.fetch_add(1, Ordering::Relaxed);
        }
    }));

    let first_ticket = runtime
        .submit_agent_command(first_window, snapshot_request())
        .unwrap();
    let _second_ticket = runtime
        .submit_agent_command(first_window, snapshot_request())
        .unwrap();
    let _other_window_ticket = runtime
        .submit_agent_command(second_window, snapshot_request())
        .unwrap();

    assert_eq!(wake_calls.load(Ordering::Relaxed), 2);
    assert_eq!(runtime.agent_command_queue(first_window).unwrap().len(), 2);
    assert_eq!(runtime.agent_command_queue(second_window).unwrap().len(), 1);

    runtime.close_session(first_window);
    assert_eq!(
        first_ticket
            .recv_timeout(Duration::from_millis(20))
            .unwrap(),
        Err(AgentCommandError::AppClosed)
    );
    assert_eq!(
        runtime
            .submit_agent_command(first_window, snapshot_request())
            .unwrap_err(),
        AgentSubmitError::WindowNotFound
    );

    runtime.shutdown_all();
    assert_eq!(
        runtime
            .submit_agent_command(second_window, snapshot_request())
            .unwrap_err(),
        AgentSubmitError::AppClosed
    );
}

#[test]
fn replacing_runtime_session_closes_only_the_old_command_queue() {
    let runtime = AppRuntime::new();
    let window_id = WindowId::new(27);
    let alive = Arc::new(AtomicBool::new(true));
    runtime.register_session(
        window_id,
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        alive.clone(),
    );
    let ticket = runtime
        .submit_agent_command(window_id, snapshot_request())
        .unwrap();

    runtime.register_session(
        window_id,
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        alive.clone(),
    );

    assert_eq!(
        ticket.recv_timeout(Duration::from_millis(20)).unwrap(),
        Err(AgentCommandError::AppClosed)
    );
    assert!(alive.load(Ordering::Acquire));
    assert!(runtime.agent_command_queue(window_id).unwrap().is_empty());
}

#[test]
fn perform_validation_returns_stable_typed_errors_before_mutation() {
    let mut tree = laid_out_button();
    let mut semantics = WindowSemanticState::new(WindowId::new(23));
    assert!(semantics.enable(&tree));
    let mut commands = WindowAgentState::new();
    let queue = commands.queue();

    for (request, expected) in [
        (
            perform_request(2, None, SemanticAction::Invoke),
            AgentErrorCode::StaleWindow,
        ),
        (
            perform_request(1, Some(2), SemanticAction::Invoke),
            AgentErrorCode::StaleRevision,
        ),
        (
            perform_request(1, Some(1), SemanticAction::Increment),
            AgentErrorCode::UnsupportedAction,
        ),
    ] {
        let (ticket, _) = queue.submit(request).unwrap();
        assert!(commands.drain_ready(&mut tree, &mut semantics, true));
        let error = ticket
            .recv_timeout(Duration::from_millis(20))
            .unwrap()
            .unwrap_err();
        assert_eq!(error.code(), expected);
        assert_eq!(error.code().as_str(), expected.as_str());
        assert!(!commands.has_in_flight());
    }

    let (ticket, _) = queue
        .submit(perform_request(1, Some(1), SemanticAction::Invoke))
        .unwrap();
    assert!(commands.drain_ready(&mut tree, &mut semantics, false));
    assert_eq!(
        ticket
            .recv_timeout(Duration::from_millis(20))
            .unwrap()
            .unwrap_err()
            .code(),
        AgentErrorCode::NotPresentable
    );

    let (ticket, _) = queue
        .submit(window_action_request(
            1,
            Some(1),
            AgentWindowAction::PressKey {
                key: KeyCode::A,
                modifiers: KeyMod::CTRL,
            },
        ))
        .unwrap();
    assert!(commands.drain_ready(&mut tree, &mut semantics, false));
    assert_eq!(
        ticket
            .recv_timeout(Duration::from_millis(20))
            .unwrap()
            .unwrap_err()
            .code(),
        AgentErrorCode::NotPresentable,
        "window input must fail before dispatch while the surface cannot present"
    );
}

#[test]
fn window_actions_share_system_events_fifo_and_the_settle_barrier() {
    let invoked = Rc::new(Cell::new(0usize));
    let invoked_for_handler = invoked.clone();
    let mut tree = ViewAdapter::build_nodes(
        button("Run")
            .on_click_fn(move || invoked_for_handler.set(invoked_for_handler.get() + 1))
            .automation_id("run"),
    );
    tree.root_mut()
        .expect("button root")
        .set_frame(Rect::new(0.0, 0.0, 160.0, 40.0));
    tree.layout();

    let mut semantics = WindowSemanticState::new(WindowId::new(28));
    assert!(semantics.enable(&tree));
    let mut commands = WindowAgentState::new();
    let queue = commands.queue();
    let (click_ticket, first_wake) = queue
        .submit(window_action_request(
            1,
            Some(1),
            AgentWindowAction::ClickAt {
                position: Point::new(80.0, 20.0),
            },
        ))
        .unwrap();
    let (key_ticket, second_wake) = queue
        .submit(window_action_request(
            1,
            None,
            AgentWindowAction::PressKey {
                key: KeyCode::Enter,
                modifiers: KeyMod::NONE,
            },
        ))
        .unwrap();
    let (snapshot_ticket, third_wake) = queue.submit(snapshot_request()).unwrap();

    assert!(first_wake);
    assert!(!second_wake);
    assert!(!third_wake);
    assert!(commands.drain_ready(&mut tree, &mut semantics, true));
    assert_eq!(invoked.get(), 2, "click and focused key use normal events");
    assert_eq!(queue.len(), 1, "snapshot stays behind both window actions");
    assert!(commands.has_in_flight());

    let _ = semantics.refresh(&tree);
    assert!(commands.finish_or_defer(&semantics, false));
    for ticket in [click_ticket, key_ticket] {
        assert!(matches!(
            ticket
                .recv_timeout(Duration::from_millis(20))
                .unwrap()
                .unwrap(),
            AgentCommandResponse::Performed {
                revision: 2,
                settled: true,
                ..
            }
        ));
    }

    assert!(commands.drain_ready(&mut tree, &mut semantics, true));
    assert!(matches!(
        snapshot_ticket
            .recv_timeout(Duration::from_millis(20))
            .unwrap()
            .unwrap(),
        AgentCommandResponse::Snapshot(WindowSemanticSnapshot { revision: 2, .. })
    ));
    assert!(!commands.has_work());
}

#[test]
fn window_input_settles_even_when_no_node_consumes_the_event() {
    let mut tree = laid_out_button();
    let root = tree.root_id().expect("button root");
    tree.set_focus(Some(root));
    let mut semantics = WindowSemanticState::new(WindowId::new(29));
    assert!(semantics.enable(&tree));
    let mut commands = WindowAgentState::new();
    let (ticket, _) = commands
        .queue()
        .submit(window_action_request(
            1,
            Some(1),
            AgentWindowAction::ClickAt {
                position: Point::new(500.0, 500.0),
            },
        ))
        .unwrap();

    assert!(commands.drain_ready(&mut tree, &mut semantics, true));
    assert!(commands.has_in_flight());
    assert!(semantics.refresh(&tree), "the real miss clears focus");
    assert!(commands.finish_or_defer(&semantics, false));
    assert!(matches!(
        ticket
            .recv_timeout(Duration::from_millis(20))
            .unwrap()
            .unwrap(),
        AgentCommandResponse::Performed {
            revision: 2,
            settled: true,
            ..
        }
    ));
    assert!(!semantics.snapshot().unwrap().nodes[0].focused);
}

#[test]
fn snapshot_after_perform_is_a_fifo_settle_barrier() {
    let invoked = Rc::new(RefCell::new(0usize));
    let invoked_for_handler = invoked.clone();
    let mut tree = ViewAdapter::build_nodes(
        button("Run")
            .on_click_fn(move || *invoked_for_handler.borrow_mut() += 1)
            .automation_id("run"),
    );
    tree.root_mut()
        .expect("button root")
        .set_frame(Rect::new(0.0, 0.0, 160.0, 40.0));
    tree.layout();

    let mut semantics = WindowSemanticState::new(WindowId::new(24));
    assert!(semantics.enable(&tree));
    let mut commands = WindowAgentState::new();
    let queue = commands.queue();
    let (perform_ticket, first_wake) = queue
        .submit(perform_request(1, Some(1), SemanticAction::Invoke))
        .unwrap();
    let (snapshot_ticket, second_wake) = queue.submit(snapshot_request()).unwrap();

    assert!(first_wake);
    assert!(!second_wake);
    assert!(commands.drain_ready(&mut tree, &mut semantics, true));
    assert!(commands.has_in_flight());
    assert_eq!(
        queue.len(),
        1,
        "snapshot must remain behind the action barrier"
    );
    assert_eq!(*invoked.borrow(), 1);

    assert!(semantics.refresh(&tree));
    assert!(commands.finish_or_defer(&semantics, false));
    let performed = perform_ticket
        .recv_timeout(Duration::from_millis(20))
        .unwrap()
        .unwrap();
    assert!(matches!(
        performed,
        AgentCommandResponse::Performed {
            revision: 2,
            presented_revision: 0,
            settled: true,
            ..
        }
    ));

    assert!(commands.drain_ready(&mut tree, &mut semantics, true));
    let snapshot = snapshot_ticket
        .recv_timeout(Duration::from_millis(20))
        .unwrap()
        .unwrap();
    assert!(matches!(
        snapshot,
        AgentCommandResponse::Snapshot(WindowSemanticSnapshot {
            revision: 2,
            presented_revision: 0,
            ..
        })
    ));
    assert!(!commands.has_work());
}

#[test]
fn real_window_turn_settles_action_then_serves_snapshot_without_extra_polling() {
    let root = button("Run").automation_id("run");
    let mut session = WindowSession::from_root_for_window(
        WindowId::new(25),
        root,
        Box::new(NullEngine::new()),
        320,
        160,
    );
    let queue = session.agent_command_queue();
    let (perform_ticket, _) = queue
        .submit(perform_request(1, None, SemanticAction::Focus))
        .unwrap();
    let (snapshot_ticket, _) = queue.submit(snapshot_request()).unwrap();

    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(2);
    platform.event_source.state.exit_after_timeout_calls = Some(1);
    let mut window = FakeWindow::new(25, "agent", 320, 160);
    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());

    let status = run_window_session_loop(
        &mut platform,
        &mut window,
        &mut session,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        None,
        map_ui_event,
        |_| false,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    let performed = perform_ticket
        .recv_timeout(Duration::from_millis(20))
        .unwrap()
        .unwrap();
    let snapshot = snapshot_ticket
        .recv_timeout(Duration::from_millis(20))
        .unwrap()
        .unwrap();
    let AgentCommandResponse::Performed {
        revision,
        presented_revision,
        settled,
        ..
    } = performed
    else {
        panic!("expected performed response");
    };
    let AgentCommandResponse::Snapshot(snapshot) = snapshot else {
        panic!("expected snapshot response");
    };
    assert!(settled);
    assert!(revision >= 2);
    assert!(
        presented_revision < revision,
        "perform response must precede present: presented={presented_revision}, revision={revision}"
    );
    assert_eq!(snapshot.revision, revision);
    assert_eq!(snapshot.presented_revision, revision);
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
}

#[test]
fn synchronous_settle_has_a_hard_pass_limit() {
    let mut tree = laid_out_button();
    let mut semantics = WindowSemanticState::new(WindowId::new(26));
    assert!(semantics.enable(&tree));
    let mut commands = WindowAgentState::new();
    let (ticket, _) = commands
        .queue()
        .submit(perform_request(1, Some(1), SemanticAction::Invoke))
        .unwrap();
    assert!(commands.drain_ready(&mut tree, &mut semantics, true));

    for _ in 0..MAX_AGENT_SETTLE_PASSES {
        commands.finish_or_defer(&semantics, true);
    }

    assert_eq!(
        ticket
            .recv_timeout(Duration::from_millis(20))
            .unwrap()
            .unwrap_err(),
        AgentCommandError::DidNotSettle {
            passes: MAX_AGENT_SETTLE_PASSES,
        }
    );
    assert!(!commands.has_work());
}
