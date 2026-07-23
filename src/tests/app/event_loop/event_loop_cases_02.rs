#[test]
fn frame_opportunity_unregisters_hidden_animation_without_tick() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let remaining = Arc::new(AtomicUsize::new(2));
    let updates = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(TestAnimatedWidget::new(remaining.clone(), updates.clone())),
        Box::new(Renderer::test()),
        800,
        600,
    );
    let root = {
        let (tree, _) = session.tree_and_engine_mut();
        let root = tree.root_id().unwrap();
        tree.get_mut(root).unwrap().set_visible(false);
        root
    };
    session
        .active_work_mut()
        .register_open(ActiveWorkKind::Animation(root));
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
    assert_eq!(updates.load(Ordering::Relaxed), 0);
    assert_eq!(remaining.load(Ordering::Relaxed), 2);
    assert!(session.active_work().is_empty());
}

#[test]
fn frame_opportunity_advances_registered_and_discovers_unregistered_animations() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let due_remaining = Arc::new(AtomicUsize::new(2));
    let due_updates = Arc::new(AtomicUsize::new(0));
    let future_remaining = Arc::new(AtomicUsize::new(2));
    let future_updates = Arc::new(AtomicUsize::new(0));
    let discovered_remaining = Arc::new(AtomicUsize::new(2));
    let discovered_updates = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(Renderer::test()),
        800,
        600,
    );
    let (due_id, future_id) = {
        let (tree, _) = session.tree_and_engine_mut();
        let root = tree.root_id().unwrap();
        let due_id = tree.add_child(
            root,
            Box::new(TestAnimatedWidget::new(
                due_remaining.clone(),
                due_updates.clone(),
            )),
        );
        let future_id = tree.add_child(
            root,
            Box::new(TestAnimatedWidget::new(
                future_remaining.clone(),
                future_updates.clone(),
            )),
        );
        let discovered_id = tree.add_child(
            root,
            Box::new(TestAnimatedWidget::new(
                discovered_remaining.clone(),
                discovered_updates.clone(),
            )),
        );
        tree.get_mut(due_id)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 100.0, 50.0));
        tree.get_mut(due_id).unwrap().set_active(true);
        tree.get_mut(future_id)
            .unwrap()
            .set_frame(Rect::new(0.0, 60.0, 100.0, 50.0));
        tree.get_mut(future_id).unwrap().set_active(true);
        tree.get_mut(discovered_id)
            .unwrap()
            .set_frame(Rect::new(0.0, 120.0, 100.0, 50.0));
        tree.get_mut(discovered_id).unwrap().set_active(true);
        tree.reset_invalidation();
        (due_id, future_id)
    };
    session
        .active_work_mut()
        .register_open(ActiveWorkKind::Animation(due_id));
    session
        .active_work_mut()
        .register_open(ActiveWorkKind::Animation(future_id));
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
    assert_eq!(due_updates.load(Ordering::Relaxed), 1);
    assert_eq!(due_remaining.load(Ordering::Relaxed), 1);
    assert_eq!(future_updates.load(Ordering::Relaxed), 1);
    assert_eq!(future_remaining.load(Ordering::Relaxed), 1);
    assert_eq!(discovered_updates.load(Ordering::Relaxed), 1);
    assert_eq!(discovered_remaining.load(Ordering::Relaxed), 1);
}

#[test]
fn app_timer_due_work_does_not_add_an_animation_tick() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let remaining = Arc::new(AtomicUsize::new(2));
    let updates = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(TestAnimatedWidget::new(remaining.clone(), updates.clone())),
        Box::new(Renderer::test()),
        800,
        600,
    );
    let root = {
        let (tree, _) = session.tree_and_engine_mut();
        tree.root_id().unwrap()
    };
    session
        .active_work_mut()
        .register_open(ActiveWorkKind::Animation(root));
    let app_timers = crate::app::app_timer::AppTimerQueue::with_clock(clock.clone());
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
    assert_eq!(fired.load(Ordering::Relaxed), 1);
    assert_eq!(updates.load(Ordering::Relaxed), 1);
    assert_eq!(remaining.load(Ordering::Relaxed), 1);
    assert!(!session.active_work().is_empty());
}

#[test]
fn finished_animation_returns_to_deep_idle_without_timeout() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let remaining = Arc::new(AtomicUsize::new(1));
    let updates = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(TestAnimatedWidget::new(remaining, updates.clone())),
        Box::new(Renderer::test()),
        800,
        600,
    );
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
    assert_eq!(updates.load(Ordering::Relaxed), 1);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert!(session.active_work().is_empty());
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);
}

#[test]
fn deep_idle_waits_without_fixed_timeout_or_extra_present() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(5);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut engine = Renderer::test();
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(Container::new()));
    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());
    let metrics = Cell::new(RenderMetrics::default());

    let status = run_widget_loop(
        &mut platform,
        &mut window,
        &mut engine,
        &mut tree,
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
    assert_eq!(stats.present_calls, 1);
    assert_eq!(window.presenter.state.present_calls.len(), 1);
    assert_eq!(stats.idle_frames, 0);
    assert_eq!(platform.text_input.state.start_calls, 0);
    assert_eq!(platform.text_input.state.stop_calls, 0);
}

#[test]
fn cpu_renderer_first_frame_present_forwards_full_damage_to_fake_presenter() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 120, 80);
    let mut session = WindowSession::from_root(
        button("hover target").into(),
        Box::new(Renderer::cpu()),
        120,
        80,
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
        |tree, _, _| {
            let root = tree.root_id().expect("root should exist");
            tree.reset_invalidation();
            tree.invalidate_paint_rect(root, Rect::new(3.0, 4.0, 5.0, 6.0));
        },
    );

    assert_eq!(status, 0);
    assert_eq!(metrics.get().present_calls, 1);
    assert_eq!(window.presenter.state.present_calls.len(), 1);
    assert_eq!(
        window
            .presenter
            .state
            .present_calls
            .last()
            .map(|call| &call.damage),
        Some(&PresentDamage::Full)
    );
}

#[test]
fn cpu_renderer_after_first_frame_forwards_partial_damage_to_fake_presenter() {
    let mut platform = FakePlatform::new();
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::key_up(KeyCode::Enter, KeyMod::NONE));
    platform.event_source.state.exit_after_blocking_calls = Some(2);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 120, 80);
    let mut session = WindowSession::from_root(
        button("hover target").into(),
        Box::new(Renderer::cpu()),
        120,
        80,
    );
    let app_state = AppState::new();
    session.set_app_state(app_state.clone());
    let root_id = session
        .tree_and_engine_mut()
        .0
        .root_id()
        .expect("root should exist");
    app_state.set_event_loop_waker(platform.event_loop().waker());
    let handle = app_state
        .get_handle(root_id)
        .expect("lookup handle should be registered");
    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());
    let metrics = Cell::new(RenderMetrics::default());
    let runtime_ticks = Cell::new(0);

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
        Some(&metrics),
        |_| None,
        |_| false,
        |_, _| {
            if runtime_ticks.get() == 1 {
                handle.invalidate();
            }
            runtime_ticks.set(runtime_ticks.get() + 1);
        },
        |_, _| {},
        || None,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(metrics.get().present_calls, 2);
    assert_eq!(window.presenter.state.present_calls.len(), 2);
    assert_eq!(
        window.presenter.state.present_calls[0].damage,
        PresentDamage::Full
    );
    assert!(matches!(
        window.presenter.state.present_calls[1].damage,
        PresentDamage::Partial(ref rects)
            if rects.len() == 1 && rects[0].2 > 0 && rects[0].3 > 0
    ));
}

#[test]
fn app_state_lookup_emit_drains_in_event_loop_and_returns_deep_idle() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let app_state = AppState::new();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut root: ViewNode = button("emit target").into();
    {
        let calls = calls.clone();
        root.handlers.push(crate::ui::HandlerRegistration::new(
            SemanticKind::Change,
            Box::new(move |event| {
                calls.lock().unwrap_or_else(|e| e.into_inner()).push((
                    event.target,
                    event.current_target,
                    event.text_payload().map(str::to_string),
                ));
            }),
        ));
    }
    let mut session = WindowSession::from_root(root, Box::new(Renderer::test()), 800, 600);
    session.set_app_state(app_state.clone());
    let root_id = session
        .tree_and_engine_mut()
        .0
        .root_id()
        .expect("root should exist");
    app_state.set_event_loop_waker(platform.event_loop().waker());
    let handle = app_state
        .get_handle(root_id)
        .expect("lookup handle should be registered");
    assert_eq!(
        handle.emit(SemanticEvent::change(root_id, "from-loop")),
        EventResult::Handled
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
    assert_eq!(platform.event_source.wake_count(), 1);
    assert_eq!(
        *calls.lock().unwrap_or_else(|e| e.into_inner()),
        vec![(root_id, root_id, Some("from-loop".to_string()))]
    );
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);
}

#[test]
fn focus_handle_wakes_target_event_loop_and_returns_deep_idle() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let app_state = AppState::new();
    let focus = crate::ui::FocusHandle::new();
    let root: ViewNode = button("focus target").into();
    let root = root.focus_handle(&focus);
    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(root, Box::new(Renderer::test()), 800, 600);
    session.set_app_state(app_state.clone());
    let root_id = session
        .tree_and_engine_mut()
        .0
        .root_id()
        .expect("root should exist");
    app_state.set_event_loop_waker(platform.event_loop().waker());
    assert_eq!(focus.focus(), Ok(()));

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
    assert_eq!(platform.event_source.wake_count(), 1);
    assert_eq!(
        session
            .tree_and_engine_mut()
            .0
            .managers()
            .focus
            .focused_component(),
        Some(root_id)
    );
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);
}

#[test]
fn key_event_after_first_frame_does_not_force_layout_without_invalidation() {
    let mut platform = FakePlatform::new();
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::key_up(KeyCode::Enter, KeyMod::NONE));
    platform.event_source.state.exit_after_blocking_calls = Some(2);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(Renderer::test()),
        800,
        600,
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
        map_ui_event,
        |_| false,
        |_, _, _| {},
    );

    let stats = metrics.get();
    assert_eq!(status, 0);
    assert_eq!(stats.layout_calls, 1);
    assert_eq!(stats.present_calls, 1);
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);
}

#[test]
fn fake_platform_pointer_click_reaches_handler_table() {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_handler = hits.clone();
    let mut platform = FakePlatform::new();
    platform.event_source.inject_all([
        UiEvent::pointer_down(Point::new(4.0, 4.0), MouseButton::Left),
        UiEvent::pointer_up(Point::new(4.0, 4.0), MouseButton::Left),
    ]);
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        button("Hit")
            .on_click_fn(move || {
                hits_for_handler.fetch_add(1, Ordering::Relaxed);
            })
            .build(),
        Box::new(Renderer::test()),
        800,
        600,
    );
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
    assert_eq!(platform.event_source.processed_count(), 2);
    assert_eq!(hits.load(Ordering::Relaxed), 1);
}

#[test]
fn fake_platform_file_drop_reaches_handler_table() {
    let dropped = Arc::new(Mutex::new(Vec::<String>::new()));
    let dropped_for_handler = dropped.clone();
    let mut platform = FakePlatform::new();
    platform.event_source.inject(UiEvent::file_drop(
        vec!["a.txt".to_string(), "b.txt".to_string()],
        Point::new(20.0, 20.0),
    ));
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut root = ViewNode::leaf(Container::new());
    root.handlers.push(crate::ui::HandlerRegistration::new(
        SemanticKind::FileDrop,
        Box::new(move |event| {
            if let Some((files, _position)) = event.file_drop_payload() {
                *dropped_for_handler
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = files.to_vec();
            }
        }),
    ));
    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(root, Box::new(Renderer::test()), 800, 600);
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
    assert_eq!(platform.event_source.processed_count(), 1);
    assert_eq!(
        &*dropped.lock().unwrap_or_else(|e| e.into_inner()),
        &vec!["a.txt".to_string(), "b.txt".to_string()]
    );
}

#[test]
fn minimized_animation_keeps_registration_without_timeout_or_frame() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let remaining = Arc::new(AtomicUsize::new(2));
    let updates = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform.event_source.inject(UiEvent::new(
        UiEventType::WindowMinimize,
        UiEventPayload::None,
    ));
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(TestAnimatedWidget::new(remaining, updates.clone())),
        Box::new(Renderer::test()),
        800,
        600,
    );
    let root = session.tree_and_engine_mut().0.root_id().unwrap();
    session
        .active_work_mut()
        .register_open(ActiveWorkKind::Animation(root));
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
        map_ui_event,
        |_| false,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(updates.load(Ordering::Relaxed), 0);
    assert_eq!(metrics.get().present_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 1);
    assert!(!session.active_work().is_empty());
    assert_eq!(session.loop_state(), WindowLoopState::RegisteredActive);
}

#[test]
fn hidden_animation_keeps_dirty_and_registration_without_wake_or_frame() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let remaining = Arc::new(AtomicUsize::new(2));
    let updates = Arc::new(AtomicUsize::new(0));
    let visible = Arc::new(AtomicBool::new(true));
    let mut platform = FakePlatform::new();
    platform.event_source.inject(UiEvent::window_hide());
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window =
        FakeWindow::new(1, "test", 800, 600).with_visibility_signal(Arc::clone(&visible));
    let mut session = WindowSession::from_root(
        ViewNode::leaf(TestAnimatedWidget::new(remaining, updates.clone())),
        Box::new(Renderer::test()),
        800,
        600,
    );
    let root = session.tree_and_engine_mut().0.root_id().unwrap();
    session
        .active_work_mut()
        .register_open(ActiveWorkKind::Animation(root));
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
        map_ui_event,
        {
            let visible = Arc::clone(&visible);
            move |event| {
                if event.type_ == UiEventType::WindowHide {
                    visible.store(false, Ordering::Relaxed);
                }
                false
            }
        },
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(updates.load(Ordering::Relaxed), 0);
    assert_eq!(metrics.get().layout_calls, 0);
    assert_eq!(metrics.get().present_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 1);
    assert!(!session.active_work().is_empty());
    assert!(session.tree_and_engine_mut().0.has_render_work());
    assert_eq!(session.loop_state(), WindowLoopState::RegisteredActive);
}

#[test]
fn visibility_change_without_lifecycle_event_suspends_before_present() {
    let visible = Arc::new(AtomicBool::new(true));
    let mut platform = FakePlatform::new();
    platform
        .event_source
        .inject(UiEvent::key_up(KeyCode::Enter, KeyMod::NONE));
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window =
        FakeWindow::new(1, "test", 800, 600).with_visibility_signal(Arc::clone(&visible));
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(Renderer::test()),
        800,
        600,
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
        map_ui_event,
        {
            let visible = Arc::clone(&visible);
            move |event| {
                if event.type_ == UiEventType::KeyUp {
                    visible.store(false, Ordering::Relaxed);
                }
                false
            }
        },
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(metrics.get().layout_calls, 0);
    assert_eq!(metrics.get().paint_calls, 0);
    assert_eq!(metrics.get().present_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 1);
    assert!(session.tree_and_engine_mut().0.has_render_work());
}
