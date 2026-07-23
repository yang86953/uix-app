#[test]
fn window_session_preserves_assigned_window_id() {
    let session = WindowSession::from_root_for_window(
        WindowId::new(9),
        ViewNode::leaf(Container::new()),
        Box::new(Renderer::test()),
        800,
        600,
    );

    assert_eq!(session.window_id(), WindowId::new(9));
}

#[test]
fn external_deadline_keeps_loop_registered_active() {
    let registry = ActiveWorkRegistry::new();
    let now = Instant::now();

    assert_eq!(
        wait_loop_state(&registry, Some(now + Duration::from_millis(10))),
        WindowLoopState::RegisteredActive
    );
    assert_eq!(
        earliest_deadline(
            Some(now + Duration::from_millis(30)),
            Some(now + Duration::from_millis(10)),
        ),
        Some(now + Duration::from_millis(10))
    );
}

#[test]
fn sync_root_frame_mismatch() {
    let mut tree = WidgetTree::new();
    let rid = tree.set_root(Box::new(Container::new()));
    if let Some(root) = tree.get_mut(rid) {
        root.set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    }
    // Renderer canvas 恒为 0×0：不得把已有根尺寸压空。
    let mut engine = Renderer::test();
    sync_root_frame_to_engine(&mut tree, &mut engine);
    let root = tree.get(rid).unwrap();
    assert_eq!(root.frame(), Rect::new(0.0, 0.0, 800.0, 600.0));
}

#[test]
fn sync_root_frame_already_matched() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(Container::new()));
    let mut engine = Renderer::test();
    sync_root_frame_to_engine(&mut tree, &mut engine);
    if let Some(rid) = tree.root_id() {
        let root = tree.get(rid).unwrap();
        assert_eq!(root.frame(), Rect::new(0.0, 0.0, 0.0, 0.0));
    }
}

#[test]
fn sync_root_frame_follows_cpu_renderer_size() {
    let mut tree = WidgetTree::new();
    let rid = tree.set_root(Box::new(Container::new()));
    // bootstrap：根近空时才从引擎补齐
    if let Some(root) = tree.get_mut(rid) {
        root.set_frame(Rect::new(0.0, 0.0, 0.0, 0.0));
    }
    let mut engine = Renderer::cpu();
    engine.initialize(1000, 800).expect("init");
    sync_root_frame_to_engine(&mut tree, &mut engine);
    let root = tree.get(rid).unwrap();
    assert_eq!(root.frame(), Rect::new(0.0, 0.0, 1000.0, 800.0));
}

#[test]
fn sync_root_frame_does_not_overwrite_valid_root() {
    let mut tree = WidgetTree::new();
    let rid = tree.set_root(Box::new(Container::new()));
    if let Some(root) = tree.get_mut(rid) {
        root.set_frame(Rect::new(0.0, 0.0, 1000.0, 800.0));
    }
    let mut engine = Renderer::cpu();
    engine.initialize(800, 600).expect("init");
    sync_root_frame_to_engine(&mut tree, &mut engine);
    let root = tree.get(rid).unwrap();
    assert_eq!(
        root.frame(),
        Rect::new(0.0, 0.0, 1000.0, 800.0),
        "valid root must not be shrunk by a lagging smaller engine size"
    );
}

#[test]
fn sync_root_frame_grows_stale_root_to_larger_engine() {
    let mut tree = WidgetTree::new();
    let rid = tree.set_root(Box::new(Container::new()));
    if let Some(root) = tree.get_mut(rid) {
        root.set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    }
    let mut engine = Renderer::cpu();
    engine.initialize(1200, 900).expect("init");
    sync_root_frame_to_engine(&mut tree, &mut engine);
    let root = tree.get(rid).unwrap();
    assert_eq!(
        root.frame(),
        Rect::new(0.0, 0.0, 1200.0, 900.0),
        "stale root must grow when engine/swapchain already enlarged"
    );
}

/// 窗口 Resize 后根 frame 与 flex 内容区须跟随新尺寸，且不被 sync/layout_shrink 压回。
#[test]
fn window_resize_updates_root_and_flex_content() {
    let mut platform = FakePlatform::new();
    // 首帧 poll 即消费 Resize，避免只断言到初始 800×600。
    platform.event_source.inject(UiEvent::resize(1000, 800));
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut engine = Renderer::cpu();
    engine.initialize(800, 600).expect("init engine");

    let root_view = column([
        row([label("nav").width(200.0), label("content").flex_grow(1.0)]).flex_grow(1.0),
        label("status"),
    ])
    .flex_grow(1.0);
    let mut session = WindowSession::from_root(root_view, Box::new(engine), 800, 600);

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());
    let metrics = Cell::new(RenderMetrics::default());
    let observed = Cell::new((0.0f32, 0.0f32, 0.0f32));

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
        |tree, engine, _| {
            let rid = tree.root_id().expect("root");
            let rf = tree.get(rid).unwrap().frame();
            let main = tree.get(rid).unwrap().children()[0];
            let content = tree.get(main).unwrap().children()[1];
            let cf = tree.get(content).unwrap().frame();
            observed.set((rf.w, rf.h, cf.w));
            assert_eq!(engine.canvas_2d().width(), 1000);
            assert_eq!(engine.canvas_2d().height(), 800);
        },
    );

    assert_eq!(status, 0);
    let (rw, rh, cw) = observed.get();
    assert!(
        (rw - 1000.0).abs() < 0.5 && (rh - 800.0).abs() < 0.5,
        "root should follow window resize, got {rw}x{rh}"
    );
    assert!(
        cw > 700.0,
        "flex content should grow with window, got width {cw}"
    );
}

#[test]
fn consecutive_window_resizes_are_coalesced_without_crossing_event_barriers() {
    let window_id = WindowId::new(7);
    let mut events = Vec::new();

    push_coalesced_event(
        &mut events,
        &UiEvent::resize(900, 640).for_window(window_id),
    );
    push_coalesced_event(
        &mut events,
        &UiEvent::resize(960, 680).for_window(window_id),
    );
    assert_eq!(events.len(), 1);
    let UiEventPayload::Resize(latest) = &events[0].payload else {
        panic!("coalesced resize must keep its payload");
    };
    assert_eq!((latest.width, latest.height), (960, 680));

    push_coalesced_event(
        &mut events,
        &UiEvent::pointer_move(Point::new(12.0, 18.0)).for_window(window_id),
    );
    push_coalesced_event(
        &mut events,
        &UiEvent::resize(1_000, 720).for_window(window_id),
    );
    assert_eq!(events.len(), 3, "input must remain a resize barrier");

    let other_window = WindowId::new(8);
    push_coalesced_event(
        &mut events,
        &UiEvent::resize(1_100, 760).for_window(other_window),
    );
    assert_eq!(events.len(), 4, "resize coalescing is window-local");
}

#[test]
fn resize_burst_rebuilds_the_surface_once_at_the_latest_extent() {
    let mut platform = FakePlatform::new();
    platform.event_source.inject_all([
        UiEvent::resize(900, 640),
        UiEvent::resize(960, 680),
        UiEvent::resize(1_000, 720),
    ]);
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "resize burst", 800, 600);
    let mut engine = Renderer::cpu();
    engine.initialize(800, 600).expect("initialize engine");
    let mut session =
        WindowSession::from_root(ViewNode::leaf(Container::new()), Box::new(engine), 800, 600);

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());
    let observed = Cell::new((0, 0));

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
        |_, engine, _| observed.set(engine.logical_extent()),
    );

    assert_eq!(status, 0);
    assert_eq!(observed.get(), (1_000, 720));
    assert_eq!(window.state.resize_notify_calls, vec![(1_000, 720)]);
}

#[test]
fn widget_tree_update_advances_animation_and_marks_dirty_rect() {
    let remaining = Arc::new(AtomicUsize::new(2));
    let updates = Arc::new(AtomicUsize::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(TestAnimatedWidget::new(
        remaining.clone(),
        updates.clone(),
    )));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 50.0));
    tree.get_mut(root).unwrap().set_active(true);
    tree.reset_invalidation();

    assert!(tree.update(0.016));

    let dirty = tree.dirty_region();
    assert_eq!(updates.load(Ordering::Relaxed), 1);
    assert_eq!(remaining.load(Ordering::Relaxed), 1);
    assert_eq!(dirty.rects(), &[Rect::new(4.0, 5.0, 6.0, 7.0)]);
}

#[test]
fn unrelated_active_frame_does_not_reset_animation_clock() {
    let start = Instant::now();
    let mut last_animation_frame = start;

    let unrelated_frame = start + Duration::from_millis(5);
    let unrelated_updates = [(NodeId::new(1), false)];
    if animation_clock_should_advance(false, &unrelated_updates) {
        last_animation_frame = unrelated_frame;
    }

    let due_frame = start + Duration::from_millis(16);
    assert_eq!(
        due_frame.duration_since(last_animation_frame),
        Duration::from_millis(16)
    );
    assert!(animation_clock_should_advance(true, &[]));
}

#[test]
fn failed_frame_waits_for_recovery_deadline_without_busy_retry() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let begin_calls = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(FailFirstBeginEngine::new(begin_calls.clone())),
        800,
        600,
    );
    assert!(session.enable_semantic_tracking());
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
    assert_eq!(begin_calls.load(Ordering::Relaxed), 1);
    assert_eq!(metrics.get().present_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 1);
    assert_eq!(
        platform.event_source.state.dispatch_timeout_durations,
        vec![Duration::from_millis(8)]
    );
    assert_eq!(session.loop_state(), WindowLoopState::RegisteredActive);
    let semantic = session.semantic_snapshot().unwrap();
    assert_eq!(semantic.revision, 1);
    assert_eq!(
        semantic.presented_revision, 0,
        "a failed frame must not advance the presented semantic revision"
    );
}

#[test]
fn retained_dirty_is_presented_after_recovery_deadline() {
    let start = Instant::now();
    let clock = SteppingClock::new(start, Duration::from_millis(10));
    let begin_calls = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(FailFirstBeginEngine::new(begin_calls.clone())),
        800,
        600,
    );
    assert!(session.enable_semantic_tracking());
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
    assert_eq!(begin_calls.load(Ordering::Relaxed), 2);
    assert_eq!(metrics.get().present_calls, 1);
    let semantic = session.semantic_snapshot().unwrap();
    assert_eq!(semantic.revision, 1);
    assert_eq!(
        semantic.presented_revision, semantic.revision,
        "only the recovered successful present may advance presented_revision"
    );
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 1);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
}

#[test]
fn idle_graphics_maintenance_releases_without_an_extra_frame_or_present() {
    let start = Instant::now();
    let clock = SteppingClock::new(start, Duration::from_millis(250));
    let present_notes = Arc::new(AtomicUsize::new(0));
    let release_calls = Arc::new(AtomicUsize::new(0));
    let end_calls = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(IdleResourceMaintenanceEngine::new(
            Arc::clone(&present_notes),
            Arc::clone(&release_calls),
            Arc::clone(&end_calls),
        )),
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
    assert_eq!(present_notes.load(Ordering::Relaxed), 1);
    assert_eq!(release_calls.load(Ordering::Relaxed), 1);
    assert_eq!(end_calls.load(Ordering::Relaxed), 1);
    assert_eq!(metrics.get().present_calls, 1);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 1);
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);
}

#[test]
fn occluded_frame_waits_for_probe_deadline_without_visual_retry() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let begin_calls = Arc::new(AtomicUsize::new(0));
    let end_calls = Arc::new(AtomicUsize::new(0));
    let probe_calls = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(OccludeFirstPresentEngine::new(
            Arc::clone(&begin_calls),
            Arc::clone(&end_calls),
            Arc::clone(&probe_calls),
        )),
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
    assert_eq!(begin_calls.load(Ordering::Relaxed), 1);
    assert_eq!(end_calls.load(Ordering::Relaxed), 1);
    assert_eq!(probe_calls.load(Ordering::Relaxed), 0);
    assert_eq!(metrics.get().present_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 1);
    assert_eq!(
        platform.event_source.state.dispatch_timeout_durations,
        vec![Duration::from_millis(100)]
    );
    assert_eq!(session.loop_state(), WindowLoopState::RegisteredActive);
}

#[test]
fn occlusion_probes_without_visual_phases_then_recovers_retained_dirty() {
    let start = Instant::now();
    let clock = SteppingClock::new(start, Duration::from_millis(100));
    let remaining = Arc::new(AtomicUsize::new(2));
    let updates = Arc::new(AtomicUsize::new(0));
    let recorded_dts = Arc::new(Mutex::new(Vec::new()));
    let begin_calls = Arc::new(AtomicUsize::new(0));
    let end_calls = Arc::new(AtomicUsize::new(0));
    let probe_calls = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(TestAnimatedWidget::recording(
            remaining,
            Arc::clone(&updates),
            Arc::clone(&recorded_dts),
        )),
        Box::new(OccludeFirstPresentEngine::new(
            Arc::clone(&begin_calls),
            Arc::clone(&end_calls),
            Arc::clone(&probe_calls),
        )),
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
    assert_eq!(probe_calls.load(Ordering::Relaxed), 2);
    assert_eq!(begin_calls.load(Ordering::Relaxed), 2);
    assert_eq!(end_calls.load(Ordering::Relaxed), 2);
    assert_eq!(updates.load(Ordering::Relaxed), 2);
    assert_eq!(
        *recorded_dts
            .lock()
            .unwrap_or_else(|error| error.into_inner()),
        vec![0.0, 0.0]
    );
    assert_eq!(metrics.get().present_calls, 1);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 1);
}

#[test]
fn event_dispatch_scopes_platform_clipboard_for_widgets() {
    let mut platform = FakePlatform::new();
    platform
        .event_source
        .inject(UiEvent::key_down(KeyCode::C, KeyMod::CTRL));
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Input::new("copy").with_value("managed clipboard")),
        Box::new(Renderer::test()),
        800,
        600,
    );
    {
        let (tree, _) = session.tree_and_engine_mut();
        let root = tree.root_id();
        tree.set_focus(root);
    }
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
    assert_eq!(
        platform.clipboard.last_set_text(),
        Some("managed clipboard")
    );
}

#[test]
fn widget_tree_update_animation_nodes_advances_only_requested_ids() {
    let first_remaining = Arc::new(AtomicUsize::new(2));
    let first_updates = Arc::new(AtomicUsize::new(0));
    let second_remaining = Arc::new(AtomicUsize::new(2));
    let second_updates = Arc::new(AtomicUsize::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Container::new()));
    let first = tree.add_child(
        root,
        Box::new(TestAnimatedWidget::new(
            first_remaining.clone(),
            first_updates.clone(),
        )),
    );
    let second = tree.add_child(
        root,
        Box::new(TestAnimatedWidget::new(
            second_remaining.clone(),
            second_updates.clone(),
        )),
    );
    tree.get_mut(first)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 50.0));
    tree.get_mut(first).unwrap().set_active(true);
    tree.get_mut(second)
        .unwrap()
        .set_frame(Rect::new(0.0, 60.0, 100.0, 50.0));
    tree.get_mut(second).unwrap().set_active(true);
    tree.reset_invalidation();

    let updates = tree.update_animation_nodes([first], 0.016);

    assert_eq!(updates, vec![(first, true)]);
    assert_eq!(first_updates.load(Ordering::Relaxed), 1);
    assert_eq!(first_remaining.load(Ordering::Relaxed), 1);
    assert_eq!(second_updates.load(Ordering::Relaxed), 0);
    assert_eq!(second_remaining.load(Ordering::Relaxed), 2);
    assert_eq!(
        tree.dirty_region().rects(),
        &[Rect::new(4.0, 5.0, 6.0, 7.0)]
    );
}

#[test]
fn active_animation_arms_one_fallback_frame_request() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let remaining = Arc::new(AtomicUsize::new(2));
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
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 0);
    assert_eq!(
        platform.event_source.state.dispatch_timeout_durations,
        vec![Duration::from_nanos(1_000_000_000 / 60)]
    );
    assert!(!session.active_work().is_empty());
    assert_eq!(session.loop_state(), WindowLoopState::RegisteredActive);
}

#[test]
fn declarative_animated_uses_the_window_frame_registration() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let animated = Animated::new(0.0_f32)
        .to(1.0, 0.005, Easing::linear)
        .loop_count(2);
    let root_animated = animated.clone();
    let root_builds = Arc::new(AtomicUsize::new(0));
    let observed_root_builds = Arc::clone(&root_builds);
    let token = FrameRequestToken::new(1, 2);
    let mut platform = FakePlatform::new();
    platform.event_source.state.timeout_events.push_back(
        UiEvent::frame_opportunity(token, start + Duration::from_millis(10), None)
            .for_window(WindowId::new(1)),
    );
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(4);

    let mut window = FakeWindow::new(1, "test", 800, 600).with_native_frame_requests();
    let mut session = WindowSession::from_root_factory_for_window(
        WindowId::new(1),
        move || {
            let builds = root_builds.fetch_add(1, Ordering::Relaxed);
            assert!(
                builds < 8,
                "Animated caused an unbounded root reconcile loop"
            );
            label("fade").opacity(root_animated.value())
        },
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
    assert!((animated.value() - 1.0).abs() < 1e-6);
    assert_eq!(
        window.state.native_frame_requests,
        vec![NativeFrameRequest::after_present(token)]
    );
    assert_eq!(window.state.native_frame_presented, vec![token]);
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 1);
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 1);
    assert_eq!(metrics.get().present_calls, 2);
    assert!(observed_root_builds.load(Ordering::Relaxed) <= 3);
    assert!(session.active_work().is_empty());
}

#[test]
fn delayed_declarative_animation_waits_on_deadline_before_requesting_frames() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let animated = Animated::new(0.0_f32).to_after(30.0, 1.0, 1.0, Easing::linear);
    let root_animated = animated.clone();
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600).with_native_frame_requests();
    let mut session = WindowSession::from_root_factory_for_window(
        WindowId::new(1),
        move || label("delayed").opacity(root_animated.value()),
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
    let deadline = session
        .active_work()
        .next_deadline()
        .expect("delayed animation deadline");
    assert_eq!(
        platform.event_source.state.dispatch_timeout_durations,
        vec![deadline.duration_since(start)]
    );
    assert!(window.state.native_frame_requests.is_empty());
    assert_eq!(animated.value(), 0.0);
    assert_eq!(session.loop_state(), WindowLoopState::RegisteredActive);
}

#[test]
fn native_frame_callback_advances_animation_before_fallback_deadline() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let remaining = Arc::new(AtomicUsize::new(2));
    let updates = Arc::new(AtomicUsize::new(0));
    let recorded_dts = Arc::new(Mutex::new(Vec::new()));
    let token = FrameRequestToken::new(1, 2);
    let mut platform = FakePlatform::new();
    platform.event_source.state.timeout_events.push_back(
        UiEvent::frame_opportunity(token, start + Duration::from_millis(10), None)
            .for_window(WindowId::new(1)),
    );
    platform.event_source.state.exit_after_blocking_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600).with_native_frame_requests();
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
    assert_eq!(updates.load(Ordering::Relaxed), 2);
    assert_eq!(
        *recorded_dts
            .lock()
            .unwrap_or_else(|error| error.into_inner()),
        vec![0.0, 0.01]
    );
    assert_eq!(
        window.state.native_frame_requests,
        vec![NativeFrameRequest::after_present(token)]
    );
    assert_eq!(window.state.native_frame_presented, vec![token]);
    assert!(window.state.cancelled_native_frame_requests.is_empty());
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 1);
    assert_eq!(platform.event_source.state.dispatch_blocking_calls, 1);
    assert_eq!(metrics.get().present_calls, 2);
    assert!(session.active_work().is_empty());
}

#[test]
fn frame_opportunity_advances_all_open_animation_registrations() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let first_remaining = Arc::new(AtomicUsize::new(2));
    let first_updates = Arc::new(AtomicUsize::new(0));
    let second_remaining = Arc::new(AtomicUsize::new(2));
    let second_updates = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(Renderer::test()),
        800,
        600,
    );
    let (first, second) = {
        let (tree, _) = session.tree_and_engine_mut();
        let root = tree.root_id().unwrap();
        let first = tree.add_child(
            root,
            Box::new(TestAnimatedWidget::new(
                first_remaining.clone(),
                first_updates.clone(),
            )),
        );
        let second = tree.add_child(
            root,
            Box::new(TestAnimatedWidget::new(
                second_remaining.clone(),
                second_updates.clone(),
            )),
        );
        tree.get_mut(first)
            .unwrap()
            .set_frame(Rect::new(0.0, 0.0, 100.0, 50.0));
        tree.get_mut(first).unwrap().set_active(true);
        tree.get_mut(second)
            .unwrap()
            .set_frame(Rect::new(0.0, 60.0, 100.0, 50.0));
        tree.get_mut(second).unwrap().set_active(true);
        tree.reset_invalidation();
        (first, second)
    };
    session
        .active_work_mut()
        .register_open(ActiveWorkKind::Animation(first));
    session
        .active_work_mut()
        .register_open(ActiveWorkKind::Animation(second));
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
    assert_eq!(first_updates.load(Ordering::Relaxed), 1);
    assert_eq!(first_remaining.load(Ordering::Relaxed), 1);
    assert_eq!(second_updates.load(Ordering::Relaxed), 1);
    assert_eq!(second_remaining.load(Ordering::Relaxed), 1);
    assert!(!session.active_work().is_empty());
}
