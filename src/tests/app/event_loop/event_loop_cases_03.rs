#[test]
fn exact_occlusion_query_suspends_before_visual_work_without_waiting_for_notification() {
    let visible = Arc::new(AtomicBool::new(true));
    let occluded = Arc::new(AtomicBool::new(false));
    let mut platform = FakePlatform::new();
    platform
        .event_source
        .inject(UiEvent::key_up(KeyCode::Enter, KeyMod::NONE));
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600)
        .with_visibility_signal(Arc::clone(&visible))
        .with_occlusion_signal(Arc::clone(&occluded));
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
            let occluded = Arc::clone(&occluded);
            move |event| {
                if event.type_ == UiEventType::KeyUp {
                    occluded.store(true, Ordering::Relaxed);
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

#[test]
fn occlusion_notification_blocks_without_polling_then_exposure_rebases_animation() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let remaining = Arc::new(AtomicUsize::new(1));
    let updates = Arc::new(AtomicUsize::new(0));
    let recorded_dts = Arc::new(Mutex::new(Vec::new()));
    let visible = Arc::new(AtomicBool::new(true));
    let occluded = Arc::new(AtomicBool::new(false));
    let transitions = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform
        .event_source
        .inject(UiEvent::window_occlusion_changed());
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::window_occlusion_changed());
    platform.event_source.state.exit_after_blocking_calls = Some(2);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600)
        .with_visibility_signal(Arc::clone(&visible))
        .with_occlusion_signal(Arc::clone(&occluded));
    let mut session = WindowSession::from_root(
        ViewNode::leaf(TestAnimatedWidget::recording(
            remaining,
            updates.clone(),
            recorded_dts.clone(),
        )),
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
            let occluded = Arc::clone(&occluded);
            let transitions = Arc::clone(&transitions);
            move |event| {
                if event.type_ == UiEventType::WindowOcclusionChanged {
                    let transition = transitions.fetch_add(1, Ordering::Relaxed);
                    occluded.store(transition == 0, Ordering::Relaxed);
                }
                false
            }
        },
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(transitions.load(Ordering::Relaxed), 2);
    assert_eq!(updates.load(Ordering::Relaxed), 1);
    assert_eq!(
        recorded_dts
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_slice(),
        &[0.0]
    );
    assert_eq!(metrics.get().layout_calls, 1);
    assert_eq!(metrics.get().present_calls, 1);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 2);
    assert!(session.active_work().is_empty());
    assert!(!session.tree_and_engine_mut().0.has_render_work());
}

#[test]
fn showing_hidden_window_rebases_animation_and_presents_retained_dirty_once() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let remaining = Arc::new(AtomicUsize::new(1));
    let updates = Arc::new(AtomicUsize::new(0));
    let recorded_dts = Arc::new(Mutex::new(Vec::new()));
    let visible = Arc::new(AtomicBool::new(true));
    let mut platform = FakePlatform::new();
    platform.event_source.inject(UiEvent::window_hide());
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::window_show());
    platform.event_source.state.exit_after_blocking_calls = Some(2);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window =
        FakeWindow::new(1, "test", 800, 600).with_visibility_signal(Arc::clone(&visible));
    let mut session = WindowSession::from_root(
        ViewNode::leaf(TestAnimatedWidget::recording(
            remaining,
            updates.clone(),
            recorded_dts.clone(),
        )),
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
                match event.type_ {
                    UiEventType::WindowHide => visible.store(false, Ordering::Relaxed),
                    UiEventType::WindowShow => visible.store(true, Ordering::Relaxed),
                    _ => {}
                }
                false
            }
        },
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(updates.load(Ordering::Relaxed), 1);
    assert_eq!(
        recorded_dts
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_slice(),
        &[0.0]
    );
    assert_eq!(metrics.get().layout_calls, 1);
    assert_eq!(metrics.get().present_calls, 1);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 2);
    assert!(session.active_work().is_empty());
    assert!(!session.tree_and_engine_mut().0.has_render_work());
}

#[test]
fn zero_extent_keeps_dirty_and_animation_registered_without_wake() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let remaining = Arc::new(AtomicUsize::new(2));
    let updates = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform.event_source.inject(UiEvent::resize(0, 0));
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
    assert_eq!(metrics.get().layout_calls, 0);
    assert_eq!(metrics.get().present_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 1);
    assert!(!session.active_work().is_empty());
    assert!(session.tree_and_engine_mut().0.has_render_work());
    assert_eq!(session.loop_state(), WindowLoopState::RegisteredActive);
}

#[test]
fn restore_rebases_animation_clock_and_presents_retained_dirty() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let remaining = Arc::new(AtomicUsize::new(1));
    let updates = Arc::new(AtomicUsize::new(0));
    let recorded_dts = Arc::new(Mutex::new(Vec::new()));
    let mut platform = FakePlatform::new();
    platform.event_source.inject(UiEvent::new(
        UiEventType::WindowMinimize,
        UiEventPayload::None,
    ));
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::new(
            UiEventType::WindowRestore,
            UiEventPayload::None,
        ));
    platform.event_source.state.exit_after_blocking_calls = Some(2);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(TestAnimatedWidget::recording(
            remaining,
            updates.clone(),
            recorded_dts.clone(),
        )),
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
    assert_eq!(updates.load(Ordering::Relaxed), 1);
    assert_eq!(
        recorded_dts
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_slice(),
        &[0.0]
    );
    assert_eq!(metrics.get().present_calls, 1);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 2);
    assert!(session.active_work().is_empty());
    assert!(!session.tree_and_engine_mut().0.has_render_work());
}

#[test]
fn minimized_window_preserves_dirty_until_restore() {
    let mut platform = FakePlatform::new();
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::new(
            UiEventType::WindowMinimize,
            UiEventPayload::None,
        ));
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::key_down(KeyCode::D, KeyMod::CTRL | KeyMod::SHIFT));
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::new(
            UiEventType::WindowRestore,
            UiEventPayload::None,
        ));
    platform.event_source.state.exit_after_blocking_calls = Some(4);
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
    assert_eq!(stats.present_calls, 2);
    assert_eq!(window.presenter.state.present_calls.len(), 2);
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);
}

#[test]
fn minimized_window_repaints_on_maximize() {
    // Win32：最小化后再最大化发 SIZE_MAXIMIZED（WindowMaximize + Resize），
    // 不发 WindowRestore；须恢复可见并 Present，否则客户区黑屏。
    let mut platform = FakePlatform::new();
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::new(
            UiEventType::WindowMinimize,
            UiEventPayload::None,
        ));
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::new(
            UiEventType::WindowMaximize,
            UiEventPayload::None,
        ));
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::resize(1920, 1080));
    platform.event_source.state.exit_after_blocking_calls = Some(4);
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
    // 首帧 + Maximize 恢复可见 + Resize 校正：Fake 源分次投递故为 3 次 Present。
    assert_eq!(stats.present_calls, 3);
    assert_eq!(window.presenter.state.present_calls.len(), 3);
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);
}

#[test]
fn ime_events_without_focused_component_do_not_start_text_input_or_force_frame() {
    let mut platform = FakePlatform::new();
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::ime_composition_start());
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::ime_composition_update("zh"));
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::ime_composition_end("中"));
    platform.event_source.state.exit_after_blocking_calls = Some(4);
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
    assert_eq!(platform.text_input.state.start_calls, 0);
    assert_eq!(platform.text_input.state.stop_calls, 0);
    assert!(session.active_work().is_empty());
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);
}

#[test]
fn window_session_deep_idle_records_loop_state() {
    let mut platform = FakePlatform::new();
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
        |_| None,
        |_| false,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);
}

#[test]
fn registered_active_future_deadline_waits_until_deadline() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(Renderer::test()),
        800,
        600,
    );
    session.active_work_mut().register(
        ActiveWorkKind::Timer(1),
        Instant::now() + Duration::from_secs(60),
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
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 1);
    assert!(!session.active_work().is_empty());
    assert_eq!(session.loop_state(), WindowLoopState::RegisteredActive);
}

#[test]
fn registered_active_wait_until_uses_injected_test_clock() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(Renderer::test()),
        800,
        600,
    );
    session
        .active_work_mut()
        .register(ActiveWorkKind::Timer(1), start + Duration::from_secs(60));

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
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 1);
    assert_eq!(
        platform.event_source.state.dispatch_timeout_durations,
        vec![Duration::from_secs(60)]
    );
    assert!(!session.active_work().is_empty());
    assert_eq!(session.loop_state(), WindowLoopState::RegisteredActive);
}

#[test]
fn focused_input_registers_open_ime_work_without_timeout() {
    let mut platform = FakePlatform::new();
    platform
        .event_source
        .inject(UiEvent::key_down(KeyCode::Tab, KeyMod::NONE));
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Input::new("type here")),
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

    assert_eq!(status, 0);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 1);
    assert!(session.active_work().is_empty());
    assert_eq!(session.loop_state(), WindowLoopState::RegisteredActive);
    assert_eq!(platform.text_input.state.start_calls, 1);
    assert_eq!(platform.text_input.state.stop_calls, 1);
    assert!(!platform.text_input.state.active);
    assert!(platform.text_input.state.cursor_rect.is_some());
}

#[test]
fn fake_platform_tab_then_text_input_targets_second_input_only() {
    let mut platform = FakePlatform::new();
    platform.event_source.inject_all([
        UiEvent::key_down(KeyCode::Tab, KeyMod::NONE),
        UiEvent::text_input("after-tab"),
    ]);
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::new(
            Container::new(),
            vec![
                ViewNode::leaf(Input::new("first")),
                ViewNode::leaf(Input::new("second")),
            ],
        ),
        Box::new(Renderer::test()),
        800,
        600,
    );
    let input_ids = {
        let (tree, _) = session.tree_and_engine_mut();
        tree.find_all_by_type::<Input>()
            .into_iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>()
    };
    assert_eq!(input_ids.len(), 2);
    {
        let (tree, _) = session.tree_and_engine_mut();
        tree.set_focus(Some(input_ids[0]));
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
    let (tree, _) = session.tree_and_engine_mut();
    assert_eq!(
        tree.managers().focus.focused_component(),
        Some(input_ids[1])
    );
    let input_value = |id| {
        tree.get(id)
            .expect("input node")
            .component()
            .as_any()
            .downcast_ref::<Input>()
            .expect("Input component")
            .current_value()
            .to_string()
    };
    assert_eq!(input_value(input_ids[0]), "");
    assert_eq!(input_value(input_ids[1]), "after-tab");
}
