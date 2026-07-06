use super::*;
use crate::app::active_work_registry::ActiveWorkKind;
use crate::app::app_timer::AppTimerQueue;
use crate::app::test_clock::TestClock;
use crate::app::window_session::{WindowLoopState, WindowSession};
use crate::draw::pipeline::RenderMetrics;
use crate::draw::NullEngine;
use crate::native::test_harness::{FakePlatform, FakeWindow};
use crate::native::traits::event::UiEvent;
use crate::native::traits::input::KeyMod;
use crate::ui::overlay::OverlayKind;
use crate::ui::theme::{DesignTokens, DynTokens};
use crate::ui::traits::TokenProvider;
use crate::ui::view::combinators::{dynamic_label, label};
use crate::ui::view::ViewNode;
use crate::ui::widgets::container::Container;
use crate::ui::widgets::feedback::Tooltip;
use crate::ui::widgets::Label;
use crate::ui::{WidgetAnimation, WidgetCapabilities, WidgetComponent, WidgetRender};
use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

    fn animation_dirty_rect(&self, _frame: Rect) -> Rect {
        self.dirty_rect
    }
}

#[test]
fn sync_root_frame_mismatch() {
    let mut tree = WidgetTree::new();
    let rid = tree.set_root(Box::new(Container::new()));
    if let Some(root) = tree.get_mut(rid) {
        root.set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    }
    let mut engine = NullEngine::new();
    sync_root_frame_to_engine(&mut tree, &mut engine);
    let root = tree.get(rid).unwrap();
    assert!(root.frame().w < 800.0);
    assert!(root.frame().h < 600.0);
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
    tree.reset_dirty();

    assert!(tree.update(0.016));

    let dirty = tree.dirty_region();
    assert_eq!(updates.load(Ordering::Relaxed), 1);
    assert_eq!(remaining.load(Ordering::Relaxed), 1);
    assert_eq!(dirty.rects(), &[Rect::new(4.0, 5.0, 6.0, 7.0)]);
}

#[test]
fn active_animation_registers_next_frame_deadline() {
    let start = Instant::now();
    let clock = TestClock::new(start);
    let remaining = Arc::new(AtomicUsize::new(2));
    let updates = Arc::new(AtomicUsize::new(0));
    let mut platform = FakePlatform::new();
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
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
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
    assert_eq!(platform.event_source.state.dispatch_timeout_calls, 0);
    assert!(session.active_work().is_empty());
    assert_eq!(session.loop_state(), WindowLoopState::DeepIdle);

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
