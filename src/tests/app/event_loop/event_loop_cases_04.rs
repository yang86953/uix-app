
#[test]
fn window_blur_unregisters_focused_input_ime_work() {
    let mut platform = FakePlatform::new();
    platform.event_source.inject_all([
        UiEvent::ime_composition_start(),
        UiEvent::ime_composition_end("done"),
        UiEvent {
            window_id: None,
            type_: UiEventType::WindowBlur,
            payload: UiEventPayload::None,
        },
    ]);
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Input::new("type here")),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    {
        let (tree, _) = session.tree_and_engine_mut();
        let root = tree.root_id().unwrap();
        tree.managers_mut().focus.set_focused_component(Some(root));
    }
    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());
    let metrics = Cell::new(RenderMetrics::default());

    let status = run_window_session_loop(
        &mut platform,
        &mut window,
        &mut session,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        Some(&metrics),
        map_ui_event,
        |_| false,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert!(session.active_work().is_empty());
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);
    assert_eq!(platform.text_input.state.start_calls, 1);
    assert_eq!(platform.text_input.state.stop_calls, 1);
    assert!(!platform.text_input.state.active);
}

#[test]
fn hiding_focused_input_unregisters_ime_work_in_same_frame() {
    let mut platform = FakePlatform::new();
    platform
        .event_source
        .inject(UiEvent::key_down(KeyCode::Tab, KeyMod::NONE));
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Input::new("type here")),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    let input = session.tree_and_engine_mut().0.root_id().unwrap();
    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());
    let metrics = Cell::new(RenderMetrics::default());
    let hidden = Cell::new(false);

    let status = run_window_session_loop(
        &mut platform,
        &mut window,
        &mut session,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        Some(&metrics),
        map_ui_event,
        |_| false,
        |tree, _, _| {
            if !hidden.replace(true) {
                tree.set_visible(input, false);
            }
        },
    );

    assert_eq!(status, 0);
    assert!(hidden.get());
    assert_eq!(platform.text_input.state.start_calls, 1);
    assert_eq!(platform.text_input.state.stop_calls, 1);
    assert!(!platform.text_input.state.active);
    assert!(session.active_work().is_empty());
    assert_eq!(
        session
            .tree_and_engine_mut()
            .0
            .managers()
            .focus
            .focused_component(),
        None
    );
}

#[test]
fn app_timer_due_work_uses_injected_test_clock() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let app_timers = AppTimerQueue::with_clock(clock.clone());
    let fired = Arc::new(AtomicUsize::new(0));
    let _handle = app_timers.run_after(Duration::from_millis(25), {
        let fired = fired.clone();
        move || {
            fired.fetch_add(1, Ordering::Relaxed);
        }
    });
    clock.advance(Duration::from_millis(25));

    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    session.set_app_timers(app_timers);

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());
    let metrics = Cell::new(RenderMetrics::default());

    let status = run_window_session_loop_with_clock(
        &mut platform,
        &mut window,
        &mut session,
        &font_service,
        &image_service,
        &theme,
        clock,
        &debug_mode,
        &cursor_pos,
        Some(&metrics),
        |_| None,
        |_| false,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(fired.load(Ordering::Relaxed), 1);
    assert!(platform.event_source.state.dispatch_timeout_calls <= 1);
    assert!(session.active_work().is_empty());
}

#[test]
fn delayed_tooltip_registers_timer_with_active_work() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    {
        let (tree, _) = session.tree_and_engine_mut();
        let root = tree.root_id().unwrap();
        let tooltip = tree.add_child(
            root,
            Box::new(Tooltip::new("Help").delay_ms(50).timer_id(42)),
        );
        tree.get_mut(root)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
        tree.get_mut(tooltip)
            .unwrap()
            .set_frame(Rect::new(10.0, 10.0, 80.0, 20.0));
        tree.dispatch_event(&SystemEvent::PointerMove {
            pos: Point::new(20.0, 15.0),
            mods: KeyMod::NONE,
        });
        tree.mark_full_frame_dirty();
    }

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());
    let metrics = Cell::new(RenderMetrics::default());

    let status = run_window_session_loop(
        &mut platform,
        &mut window,
        &mut session,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        Some(&metrics),
        |_| None,
        |_| false,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 1);
    assert!(!session.active_work().is_empty());
    assert_eq!(session.loop_state(), WindowLoopState::RegisteredActive);
}

#[test]
fn registered_active_due_work_drains_without_timeout() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    session.active_work_mut().register(
        ActiveWorkKind::Timer(1),
        Instant::now() - Duration::from_millis(1),
    );

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());
    let metrics = Cell::new(RenderMetrics::default());

    let status = run_window_session_loop(
        &mut platform,
        &mut window,
        &mut session,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        Some(&metrics),
        |_| None,
        |_| false,
        |_, _, _| {},
    );

    let stats = metrics.get();
    assert_eq!(status, 0);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert!(session.active_work().is_empty());
    assert_eq!(stats.present_calls, 1);
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);
}

#[test]
fn due_registry_timer_dispatches_system_timer_to_tree() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    {
        let (tree, _) = session.tree_and_engine_mut();
        let root = tree.root_id().unwrap();
        let tooltip = tree.add_child(
            root,
            Box::new(Tooltip::new("Help").delay_ms(300).timer_id(42)),
        );
        tree.get_mut(root)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
        tree.get_mut(tooltip)
            .unwrap()
            .set_frame(Rect::new(10.0, 10.0, 80.0, 20.0));
        tree.get_mut(tooltip).unwrap().set_active(true);
        let _ = tree.dispatch_event(&SystemEvent::PointerMove {
            pos: Point::new(20.0, 15.0),
            mods: KeyMod::NONE,
        });
        tree.mark_full_frame_dirty();
    }
    session.active_work_mut().register(
        ActiveWorkKind::Timer(42),
        Instant::now() - Duration::from_millis(1),
    );

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());
    let metrics = Cell::new(RenderMetrics::default());

    let status = run_window_session_loop(
        &mut platform,
        &mut window,
        &mut session,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        Some(&metrics),
        |_| None,
        |_| false,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert!(platform.event_source.state.dispatch_timeout_calls <= 1);
    assert!(!session.active_work().is_empty());
    assert_eq!(session.loop_state(), WindowLoopState::RegisteredActive);

    let (tree, _) = session.tree_and_engine_mut();
    let top = tree.overlay_stack().top().unwrap();
    assert_eq!(top.kind(), OverlayKind::Tooltip);
}

#[test]
fn theme_changed_is_ignored_without_system_theme_opt_in() {
    let mut platform = FakePlatform::new();
    platform.display.set_dark_mode(true);
    platform.event_source.inject(UiEvent::theme_changed(true));
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    let tokens = Arc::new(DynTokens::new(DesignTokens::antd_light()));
    let provider: Arc<dyn TokenProvider> = tokens.clone();
    let theme = RefCell::new(Theme::from_arc(provider));
    let font_service = FontService::new();
    let image_service = ImageService::new();
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
        |_| Some(SystemEvent::ThemeChanged { is_dark: true }),
        |_| false,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert!(!tokens.is_dark());
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
}

#[test]
fn theme_changed_opt_in_updates_tokens_from_display_without_polling() {
    let mut platform = FakePlatform::new();
    platform.display.set_dark_mode(true);
    platform.event_source.inject(UiEvent::theme_changed(false));
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    let tokens = Arc::new(DynTokens::new(DesignTokens::antd_light()));
    let provider: Arc<dyn TokenProvider> = tokens.clone();
    let theme = RefCell::new(Theme::from_arc(provider));
    let font_service = FontService::new();
    let image_service = ImageService::new();
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());

    let status = run_window_session_loop_with_system_theme(
        &mut platform,
        &mut window,
        &mut session,
        &font_service,
        &image_service,
        &theme,
        Some(tokens.as_ref()),
        &debug_mode,
        &cursor_pos,
        None,
        |_| Some(SystemEvent::ThemeChanged { is_dark: false }),
        |_| false,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert!(tokens.is_dark());
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
}

#[test]
fn theme_changed_opt_in_notifies_runtime_task_with_display_mode() {
    let mut platform = FakePlatform::new();
    platform.display.set_dark_mode(true);
    platform.event_source.inject(UiEvent::theme_changed(false));
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    let tokens = Arc::new(DynTokens::new(DesignTokens::antd_light()));
    let provider: Arc<dyn TokenProvider> = tokens.clone();
    let theme = RefCell::new(Theme::from_arc(provider));
    let font_service = FontService::new();
    let image_service = ImageService::new();
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());
    let observed = Cell::new(None);

    let status = run_window_session_loop_with_system_theme_and_tasks(
        &mut platform,
        &mut window,
        &mut session,
        &font_service,
        &image_service,
        &theme,
        Some(tokens.as_ref()),
        &debug_mode,
        &cursor_pos,
        None,
        |_| Some(SystemEvent::ThemeChanged { is_dark: false }),
        |_| false,
        |_, _| {},
        |event, _| {
            if let UiEventPayload::ThemeChanged(data) = &event.payload {
                observed.set(Some(data.is_dark));
            }
        },
        || None,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(observed.get(), Some(true));
    assert!(tokens.is_dark());
}

#[test]
fn theme_changed_without_system_theme_does_not_notify_runtime_task() {
    let mut platform = FakePlatform::new();
    platform.display.set_dark_mode(true);
    platform.event_source.inject(UiEvent::theme_changed(true));
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    let tokens = Arc::new(DynTokens::new(DesignTokens::antd_light()));
    let provider: Arc<dyn TokenProvider> = tokens.clone();
    let theme = RefCell::new(Theme::from_arc(provider));
    let font_service = FontService::new();
    let image_service = ImageService::new();
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());
    let observed = Cell::new(false);

    let status = run_window_session_loop_with_system_theme_and_tasks(
        &mut platform,
        &mut window,
        &mut session,
        &font_service,
        &image_service,
        &theme,
        None,
        &debug_mode,
        &cursor_pos,
        None,
        |_| Some(SystemEvent::ThemeChanged { is_dark: true }),
        |_| false,
        |_, _| {},
        |event, _| {
            if matches!(event.payload, UiEventPayload::ThemeChanged(_)) {
                observed.set(true);
            }
        },
        || None,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert!(!observed.get());
    assert!(!tokens.is_dark());
}

#[test]
fn app_timer_due_work_runs_callback_without_fixed_polling() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    let app_timers = crate::app::app_timer::AppTimerQueue::new();
    let fired = Arc::new(AtomicUsize::new(0));
    let _handle = app_timers.run_after(Duration::ZERO, {
        let fired = fired.clone();
        move || {
            fired.fetch_add(1, Ordering::Relaxed);
        }
    });
    session.set_app_timers(app_timers);

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
        |_| None,
        |_| false,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(fired.load(Ordering::Relaxed), 1);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert!(session.active_work().is_empty());
}

#[test]
fn consecutive_due_work_pumps_pending_events_before_next_frame() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_pending_calls = Some(2);
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    let app_timers = AppTimerQueue::with_clock(clock.clone());
    let second_fired = Arc::new(AtomicUsize::new(0));
    let _first = app_timers.run_after(Duration::ZERO, {
        let app_timers = app_timers.clone();
        let second_fired = second_fired.clone();
        move || {
            app_timers
                .run_after(Duration::ZERO, move || {
                    second_fired.fetch_add(1, Ordering::Relaxed);
                })
                .detach();
        }
    });
    session.set_app_timers(app_timers);

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());

    let status = run_window_session_loop_with_clock(
        &mut platform,
        &mut window,
        &mut session,
        &font_service,
        &image_service,
        &theme,
        clock,
        &debug_mode,
        &cursor_pos,
        None,
        |_| None,
        |_| false,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(platform.event_source.state.dispatch_pending_calls, 2);
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 0);
    assert_eq!(second_fired.load(Ordering::Relaxed), 0);
}

#[test]
fn post_to_ui_drains_after_app_timer_and_before_frame_update() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    let app_timers = crate::app::app_timer::AppTimerQueue::new();
    let main_thread_queue = crate::app::main_thread_queue::MainThreadQueue::new();
    let order = Arc::new(Mutex::new(Vec::new()));
    let frame_seen = Arc::new(AtomicUsize::new(0));

    let _timer = app_timers.run_after(Duration::ZERO, {
        let order = order.clone();
        move || {
            order
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push("timer")
        }
    });
    main_thread_queue.enqueue({
        let order = order.clone();
        move || order.lock().unwrap_or_else(|e| e.into_inner()).push("post")
    });
    session.set_app_timers(app_timers);
    session.set_main_thread_queue(main_thread_queue);

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
        |_| None,
        |_| false,
        {
            let order = order.clone();
            let frame_seen = frame_seen.clone();
            move |_, _, _| {
                frame_seen.fetch_add(1, Ordering::Relaxed);
                order
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push("frame");
            }
        },
    );

    assert_eq!(status, 0);
    assert_eq!(
        *order.lock().unwrap_or_else(|e| e.into_inner()),
        vec!["timer", "post", "frame"]
    );
    assert_eq!(frame_seen.load(Ordering::Relaxed), 1);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
}

#[test]
fn ui_event_registered_timer_drains_before_post_to_ui_in_same_frame() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform
        .event_source
        .inject(UiEvent::pointer_move(Point::new(20.0, 15.0)));

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    let app_timers = crate::app::app_timer::AppTimerQueue::new();
    let main_thread_queue = crate::app::main_thread_queue::MainThreadQueue::new();
    let order = Arc::new(Mutex::new(Vec::new()));
    let frame_seen = Arc::new(AtomicUsize::new(0));
    let timer_handles = Arc::new(Mutex::new(Vec::new()));
    main_thread_queue.enqueue({
        let order = order.clone();
        move || order.lock().unwrap_or_else(|e| e.into_inner()).push("post")
    });
    session.set_app_timers(app_timers.clone());
    session.set_main_thread_queue(main_thread_queue);

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
        {
            let app_timers = app_timers.clone();
            let order = order.clone();
            let timer_handles = timer_handles.clone();
            move |event| {
                if matches!(event.type_, UiEventType::PointerMove) {
                    order
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push("event");
                    let order = order.clone();
                    let timer = app_timers.run_after(Duration::ZERO, move || {
                        order
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push("timer");
                    });
                    timer_handles
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(timer);
                }
                None
            }
        },
        |_| false,
        {
            let order = order.clone();
            let frame_seen = frame_seen.clone();
            move |_, _, _| {
                frame_seen.fetch_add(1, Ordering::Relaxed);
                order
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push("frame");
            }
        },
    );

    assert_eq!(status, 0);
    assert_eq!(
        *order.lock().unwrap_or_else(|e| e.into_inner()),
        vec!["event", "timer", "post", "frame"]
    );
    assert_eq!(frame_seen.load(Ordering::Relaxed), 1);
    assert!(session.active_work().is_empty());
}
