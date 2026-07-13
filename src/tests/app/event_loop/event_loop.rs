use crate::tests::common::*;
use crate::ui::widgets::Container;
use crate::app::main_thread_queue::MainThreadContext;
use crate::app::clock::{system_clock, AppClock};
use crate::draw::pipeline::{ FrameRenderInput, FrameRenderer, InvalidationSource, RenderMetrics };
use crate::draw::traits::GraphicsEngine;
use crate::native::traits::platform::Platform;
use crate::native::traits::window::PlatformWindow;
use crate::ui::clipboard;
use crate::ui::core::widget::WidgetCore;
use crate::app::event_loop::event_loop::*;
use crate::app::active_work_registry::{ActiveWorkKind, ActiveWorkRegistry};
use crate::app::app_timer::AppTimerQueue;
use crate::app::map_ui_event;
use crate::tests::app::test_clock::TestClock;
use crate::app::window_session::{WindowLoopState, WindowSession};
use crate::native::test_harness::{FakePlatform, FakeWindow};
use crate::native::traits::event::{UiEvent, UiEventPayload, UiEventType};
use crate::ui::traits::TokenProvider;
use crate::ui::view::combinators::{button, dynamic_label, label};
use crate::ui::view::{View, ViewNode, column, row};
use crate::ui::widgets::Label;
use crate::ui::widgets::feedback::Tooltip;
use crate::ui::widgets::input::input::Input;
use std::any::Any;

struct TestAnimatedWidget {
    remaining_updates: Arc<AtomicUsize>,
    update_calls: Arc<AtomicUsize>,
    dirty_rect: Rect,
}

impl TestAnimatedWidget {
    fn new(remaining_updates: Arc<AtomicUsize>, update_calls: Arc<AtomicUsize>) -> Self {
        Self {
            remaining_updates,
            update_calls,
            dirty_rect: Rect::new(4.0, 5.0, 6.0, 7.0),
        }
    }
}

impl WidgetComponent for TestAnimatedWidget {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::RENDER | WidgetCapabilities::ANIMATION)
    }

    fn as_render(&self) -> Option<&dyn WidgetRender> {
        Some(self)
    }

    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        Some(self)
    }

    fn as_animation(&self) -> Option<&dyn WidgetAnimation> {
        Some(self)
    }

    fn as_animation_mut(&mut self) -> Option<&mut dyn WidgetAnimation> {
        Some(self)
    }
}

impl WidgetRender for TestAnimatedWidget {
    fn render(
        &self,
        _frame: Rect,
        _ctx: &mut crate::draw::painting::PaintContext,
        _tree: &WidgetTree,
    ) {
    }
}

impl WidgetAnimation for TestAnimatedWidget {
    fn update_animation(&mut self, _dt: f64) -> bool {
        self.update_calls.fetch_add(1, Ordering::Relaxed);
        let previous = self.remaining_updates.fetch_sub(1, Ordering::Relaxed);
        previous > 1
    }

    fn dirty_bounds(&self, _frame: Rect) -> Rect {
        self.dirty_rect
    }
}

#[test]
fn window_session_preserves_assigned_window_id() {
    let session = WindowSession::from_root_for_window(
        WindowId::new(9),
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
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
    // NullEngine canvas 恒为 0×0：不得把已有根尺寸压空。
    let mut engine = NullEngine::new();
    sync_root_frame_to_engine(&mut tree, &mut engine);
    let root = tree.get(rid).unwrap();
    assert_eq!(root.frame(), Rect::new(0.0, 0.0, 800.0, 600.0));
}

#[test]
fn sync_root_frame_already_matched() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(Container::new()));
    let mut engine = NullEngine::new();
    sync_root_frame_to_engine(&mut tree, &mut engine);
    if let Some(rid) = tree.root_id() {
        let root = tree.get(rid).unwrap();
        assert_eq!(root.frame(), Rect::new(0.0, 0.0, 0.0, 0.0));
    }
}

#[test]
fn sync_root_frame_follows_software_engine_size() {
    let mut tree = WidgetTree::new();
    let rid = tree.set_root(Box::new(Container::new()));
    // bootstrap：根近空时才从引擎补齐
    if let Some(root) = tree.get_mut(rid) {
        root.set_frame(Rect::new(0.0, 0.0, 0.0, 0.0));
    }
    let mut engine = SoftwareEngine::new();
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
    let mut engine = SoftwareEngine::new();
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
    let mut engine = SoftwareEngine::new();
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
    let mut engine = SoftwareEngine::new();
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
fn active_animation_registers_next_frame_deadline() {
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
        vec![Duration::from_millis(16)]
    );
    assert!(!session.active_work().is_empty());
    assert_eq!(session.loop_state(), WindowLoopState::RegisteredActive);
}

#[test]
fn due_animation_work_advances_only_due_node() {
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
        Box::new(NullEngine::new()),
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
        .register(ActiveWorkKind::Animation(first), start);
    session.active_work_mut().register(
        ActiveWorkKind::Animation(second),
        start + Duration::from_secs(60),
    );
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
    assert_eq!(second_updates.load(Ordering::Relaxed), 0);
    assert_eq!(second_remaining.load(Ordering::Relaxed), 2);
    assert!(!session.active_work().is_empty());
}

#[test]
fn due_animation_work_unregisters_hidden_node_without_tick() {
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
        Box::new(NullEngine::new()),
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
        .register(ActiveWorkKind::Animation(root), start);
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
fn due_animation_frame_discovers_unregistered_animation_without_advancing_future() {
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
        Box::new(NullEngine::new()),
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
        .register(ActiveWorkKind::Animation(due_id), start);
    session.active_work_mut().register(
        ActiveWorkKind::Animation(future_id),
        start + Duration::from_secs(60),
    );
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
    assert_eq!(future_updates.load(Ordering::Relaxed), 0);
    assert_eq!(future_remaining.load(Ordering::Relaxed), 2);
    assert_eq!(discovered_updates.load(Ordering::Relaxed), 1);
    assert_eq!(discovered_remaining.load(Ordering::Relaxed), 1);
}

#[test]
fn app_timer_due_work_does_not_advance_future_animation() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let remaining = Arc::new(AtomicUsize::new(2));
    let updates = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 800, 600);
    let mut session = WindowSession::from_root(
        ViewNode::leaf(TestAnimatedWidget::new(remaining.clone(), updates.clone())),
        Box::new(NullEngine::new()),
        800,
        600,
    );
    let root = {
        let (tree, _) = session.tree_and_engine_mut();
        tree.root_id().unwrap()
    };
    session.active_work_mut().register(
        ActiveWorkKind::Animation(root),
        start + Duration::from_secs(60),
    );
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
    assert_eq!(updates.load(Ordering::Relaxed), 0);
    assert_eq!(remaining.load(Ordering::Relaxed), 2);
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
    let mut engine = NullEngine::new();
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
    assert_eq!(stats.idle_frames, 4);
    assert_eq!(platform.text_input.state.start_calls, 0);
    assert_eq!(platform.text_input.state.stop_calls, 0);
}

#[test]
fn software_engine_first_frame_present_forwards_full_damage_to_fake_presenter() {
    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    platform.event_source.state.exit_after_timeout_calls = Some(1);

    let mut window = FakeWindow::new(1, "test", 120, 80);
    let mut session = WindowSession::from_root(
        button("hover target").into(),
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
fn software_engine_after_first_frame_forwards_partial_damage_to_fake_presenter() {
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
        Box::new(SoftwareEngine::new()),
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
    let mut session = WindowSession::from_root(root, Box::new(NullEngine::new()), 800, 600);
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
        Box::new(NullEngine::new()),
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
    let mut session = WindowSession::from_root(root, Box::new(NullEngine::new()), 800, 600);
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
fn hidden_window_preserves_dirty_until_restore() {
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

    let stats = metrics.get();
    assert_eq!(status, 0);
    assert_eq!(stats.present_calls, 2);
    assert_eq!(window.presenter.state.present_calls.len(), 2);
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);
}

#[test]
fn hidden_window_repaints_on_maximize() {
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
        Box::new(NullEngine::new()),
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
        Box::new(NullEngine::new()),
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
        Box::new(NullEngine::new()),
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
            .value()
            .to_string()
    };
    assert_eq!(input_value(input_ids[0]), "");
    assert_eq!(input_value(input_ids[1]), "after-tab");
}

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
fn state_set_reconciles_from_factory_once_before_frame() {
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
    assert_eq!(factory_calls.load(Ordering::Relaxed), 2);
    assert_eq!(
        *frame_factory_calls
            .lock()
            .unwrap_or_else(|e| e.into_inner()),
        vec![2]
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
    assert!(window.is_visible(), "window must show only after first present");
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
    engine.initialize(120, 80).expect("init engine to window size");
    let mut session = WindowSession::from_root(
        button("layout once").into(),
        Box::new(engine),
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
    // Session 构造时已 layout；首帧 needs_work 再 layout 一次即可。
    // 旧路径还会在 !rendered_first 再强制一次 → layout_calls≥2。
    assert_eq!(
        metrics.get().layout_calls,
        1,
        "first frame must not force a redundant second layout"
    );
}
