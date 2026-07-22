#[test]
fn pending_root_reconciles_once_before_frame_and_keeps_last_update() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root_factory(
        || label("initial"),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    let main_thread_queue = session.main_thread_queue();
    main_thread_queue.enqueue_with_context(|ctx| ctx.update_root(label("first")));
    main_thread_queue.enqueue_with_context(|ctx| ctx.update_root(label("second")));

    let frame_labels = Arc::new(Mutex::new(Vec::new()));
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
            let frame_labels = frame_labels.clone();
            move |tree, _, _| {
                let text = tree
                    .root()
                    .unwrap()
                    .component()
                    .as_any()
                    .downcast_ref::<Label>()
                    .unwrap()
                    .text()
                    .to_string();
                frame_labels
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(text);
            }
        },
    );

    assert_eq!(status, 0);
    assert_eq!(
        *frame_labels.lock().unwrap_or_else(|e| e.into_inner()),
        vec!["second".to_string()]
    );
    assert!(!session.reconcile_pending());
    assert!(session.take_pending_root().is_none());
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
}

#[test]
fn dynamic_label_state_set_paints_without_rebuilding_factory() {
    use crate::ui::State;

    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let state = State::new(0);
    let factory_calls = Arc::new(AtomicUsize::new(0));
    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root_factory(
        {
            let state = state.clone();
            let factory_calls = factory_calls.clone();
            move || {
                factory_calls.fetch_add(1, Ordering::Relaxed);
                let state = state.clone();
                dynamic_label(move || format!("value-{}", state.get()))
            }
        },
        Box::new(NullEngine::new()),
        800,
        600,
    );
    let main_thread_queue = session.main_thread_queue();
    main_thread_queue.enqueue(move || {
        state.set(1);
        state.set(2);
    });

    let frame_factory_calls = Arc::new(Mutex::new(Vec::new()));
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
            let frame_factory_calls = frame_factory_calls.clone();
            let factory_calls = factory_calls.clone();
            move |_, _, _| {
                frame_factory_calls
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(factory_calls.load(Ordering::Relaxed));
            }
        },
    );

    assert_eq!(status, 0);
    assert_eq!(factory_calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        *frame_factory_calls
            .lock()
            .unwrap_or_else(|e| e.into_inner()),
        vec![1]
    );
    assert!(!session.reconcile_pending());
    assert!(session.take_pending_root().is_none());
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
}

/// Ctrl+Shift+D 切换 debug_mode，并标脏以重绘 overlay。
#[test]
fn ctrl_shift_d_toggles_debug_mode_and_marks_dirty() {
    let mut platform = FakePlatform::new();
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::key_down(KeyCode::D, KeyMod::CTRL | KeyMod::SHIFT));
    platform.event_source.state.exit_after_blocking_calls = Some(2);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
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
    assert!(
        debug_mode.get(),
        "Ctrl+Shift+D should enable debug_mode for overlay borders / HUD"
    );
}

/// debug 开启后：同一 hit 上连续 PointerMove 不得每帧全脏；换目标才标脏。
#[test]
fn debug_pointer_move_dirties_only_when_hover_target_changes() {
    let mut platform = FakePlatform::new();
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::key_down(KeyCode::D, KeyMod::CTRL | KeyMod::SHIFT));
    // 同一根 Container 上的两点 → hit 不变，第二次 move 不应再标脏。
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::pointer_move(Point::new(40.0, 40.0)));
    platform
        .event_source
        .state
        .blocking_events
        .push_back(UiEvent::pointer_move(Point::new(48.0, 42.0)));
    platform.event_source.state.exit_after_blocking_calls = Some(4);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
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
    assert!(debug_mode.get());
    let presents = metrics.get().present_calls;
    // 首帧 + 开 debug + 首次 hover 变化 ≤ 3；若每 move 都脏会 ≥ 4。
    assert!(
        presents <= 3,
        "same-hit moves must not full-dirty each time, present_calls={presents}"
    );
}

#[test]
fn deferred_show_reveals_window_only_after_first_present() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 120, 80);
    assert!(!window.is_visible());
    assert_eq!(window.state.show_calls, 0);

    let mut session = WindowSession::from_root(
        button("ready").into(),
        Box::new(SoftwareEngine::new()),
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
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert_eq!(metrics.get().present_calls, 1);
    assert!(
        window.is_visible(),
        "window must show only after first present"
    );
    assert_eq!(window.state.show_calls, 1);
    assert_eq!(window.state.raise_calls, 1);
}

#[test]
fn first_frame_skips_redundant_forced_layout_when_already_laid_out() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 120, 80);
    let mut engine = SoftwareEngine::new();
    engine
        .initialize(120, 80)
        .expect("init engine to window size");
    let mut session =
        WindowSession::from_root(button("layout once").into(), Box::new(engine), 120, 80);
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
    assert_eq!(metrics.get().present_calls, 1);
    // Session 构造时已 layout；首帧 needs_work 再 layout 一次即可。
    // 旧路径还会在 !rendered_first 再强制一次 → layout_calls≥2。
    assert_eq!(
        metrics.get().layout_calls,
        1,
        "first frame must not force a redundant second layout"
    );
}

#[test]
fn image_loading_invalidations_created_during_paint_survive_present() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "image-loading", 160, 120);
    let mut engine = SoftwareEngine::new();
    engine.initialize(160, 120).expect("software engine");
    let mut session = WindowSession::from_root(
        ViewNode::leaf(
            Image::new(80.0, 48.0)
                .src("assets/images/demo.png")
                .placeholder(label("loading"))
                .preview(false),
        ),
        Box::new(engine),
        160,
        120,
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
        |_| None,
        |_| false,
        |_, _, _| {},
    );

    assert_eq!(status, 0);
    assert!(
        image_service.memory_usage() > 0,
        "the paint-time Loading invalidation must schedule the subsequent decode frame"
    );
    let (tree, _) = session.tree_and_engine_mut();
    let placeholder = tree.find_by_type::<Label>().expect("placeholder child");
    assert!(
        !tree.is_effectively_visible(placeholder),
        "the paint-time Ready invalidation must schedule the layout that hides the placeholder"
    );
}

#[test]
fn avatar_first_load_invalidation_created_during_paint_survives_present() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "avatar-loading", 160, 120);
    let mut engine = SoftwareEngine::new();
    engine.initialize(160, 120).expect("software engine");
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Avatar::new("Ada").src("assets/images/demo.png")),
        Box::new(engine),
        160,
        120,
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
    assert!(image_service.memory_usage() > 0);
    assert!(
        metrics.get().present_calls >= 2,
        "Avatar's first-load paint invalidation must receive a settling frame"
    );
}
