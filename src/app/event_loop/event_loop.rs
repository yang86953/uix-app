//! Render Loop — OS 事件 + Widget 调度；渲染段委托 draw FrameRenderer。

use crate::app::active_work_registry::{ActiveWorkKind, ActiveWorkRegistry};
use crate::app::main_thread_queue::MainThreadContext;
use crate::app::test_clock::{system_clock, AppClock};
use crate::app::window_session::{WindowLoopState, WindowSession};
use crate::core::{Point, Rect};
use crate::draw::font::font_service::FontService;
use crate::draw::image::ImageService;
use crate::draw::painting::ThemeSnapshot;
use crate::draw::pipeline::{
    FrameRenderInput, FrameRenderer, InvalidationSource, NodeId, RenderMetrics,
};
use crate::draw::traits::GraphicsEngine;
use crate::draw::RenderOutcome;
use crate::native::traits::event::{UiEvent, UiEventPayload, UiEventType};
use crate::native::traits::platform::Platform;
use crate::native::traits::window::PlatformWindow;
use crate::ui::clipboard;
use crate::ui::core::widget::WidgetCore;
use crate::ui::theme::{DynTokens, Theme};
use crate::ui::{ComponentId, EventResult, SystemEvent, WidgetTree};
use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

const ANIMATION_FRAME_INTERVAL: Duration = Duration::from_millis(16);

fn report_resize_notify_error(context: &str, result: crate::core::Result<()>) {
    if let Err(error) = result {
        crate::core::log::warn_fn(format!("{context}: {}", error.short_what()));
    }
}

/// 运行完整的 widget 渲染事件循环。
#[allow(clippy::too_many_arguments)]
pub fn run_widget_loop<M, X, F>(
    platform: &mut dyn Platform,
    platform_window: &mut dyn PlatformWindow,
    engine: &mut dyn GraphicsEngine,
    tree: &mut WidgetTree,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    metrics: Option<&Cell<RenderMetrics>>,
    map_event: M,
    on_exit: X,
    on_frame: F,
) -> i32
where
    M: Fn(&UiEvent) -> Option<SystemEvent>,
    X: Fn(&UiEvent) -> bool,
    F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn Platform),
{
    let mut active_work = ActiveWorkRegistry::new();
    let mut pending_root = None;
    let mut reconcile_pending = false;
    run_widget_loop_with_active_work(
        platform,
        platform_window,
        engine,
        tree,
        &mut active_work,
        crate::app::app_timer::AppTimerQueue::new(),
        crate::app::main_thread_queue::MainThreadQueue::new(),
        system_clock(),
        None,
        &mut pending_root,
        &mut reconcile_pending,
        None,
        font_service,
        image_service,
        theme,
        None,
        debug_mode,
        cursor_pos,
        metrics,
        map_event,
        on_exit,
        |_| {},
        |_, _| {},
        || None,
        on_frame,
    )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_window_session_loop<M, X, F>(
    platform: &mut dyn Platform,
    platform_window: &mut dyn PlatformWindow,
    session: &mut WindowSession,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    metrics: Option<&Cell<RenderMetrics>>,
    map_event: M,
    on_exit: X,
    on_frame: F,
) -> i32
where
    M: Fn(&UiEvent) -> Option<SystemEvent>,
    X: Fn(&UiEvent) -> bool,
    F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn Platform),
{
    run_window_session_loop_with_system_theme(
        platform,
        platform_window,
        session,
        font_service,
        image_service,
        theme,
        None,
        debug_mode,
        cursor_pos,
        metrics,
        map_event,
        on_exit,
        on_frame,
    )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_window_session_loop_with_system_theme<M, X, F>(
    platform: &mut dyn Platform,
    platform_window: &mut dyn PlatformWindow,
    session: &mut WindowSession,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    system_theme_tokens: Option<&DynTokens>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    metrics: Option<&Cell<RenderMetrics>>,
    map_event: M,
    on_exit: X,
    on_frame: F,
) -> i32
where
    M: Fn(&UiEvent) -> Option<SystemEvent>,
    X: Fn(&UiEvent) -> bool,
    F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn Platform),
{
    run_window_session_loop_with_system_theme_and_tasks(
        platform,
        platform_window,
        session,
        font_service,
        image_service,
        theme,
        system_theme_tokens,
        debug_mode,
        cursor_pos,
        metrics,
        map_event,
        on_exit,
        |_| {},
        |_, _| {},
        || None,
        on_frame,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_window_session_loop_with_system_theme_and_tasks<M, X, T, R, D, F>(
    platform: &mut dyn Platform,
    platform_window: &mut dyn PlatformWindow,
    session: &mut WindowSession,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    system_theme_tokens: Option<&DynTokens>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    metrics: Option<&Cell<RenderMetrics>>,
    map_event: M,
    on_exit: X,
    on_runtime_tasks: T,
    on_foreign_event: R,
    next_external_deadline: D,
    on_frame: F,
) -> i32
where
    M: Fn(&UiEvent) -> Option<SystemEvent>,
    X: Fn(&UiEvent) -> bool,
    T: FnMut(&mut dyn Platform),
    R: FnMut(&UiEvent, &mut dyn Platform),
    D: FnMut() -> Option<Instant>,
    F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn Platform),
{
    run_window_session_loop_with_system_theme_and_clock(
        platform,
        platform_window,
        session,
        font_service,
        image_service,
        theme,
        system_theme_tokens,
        system_clock(),
        debug_mode,
        cursor_pos,
        metrics,
        map_event,
        on_exit,
        on_runtime_tasks,
        on_foreign_event,
        next_external_deadline,
        on_frame,
    )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_window_session_loop_with_clock<M, X, F>(
    platform: &mut dyn Platform,
    platform_window: &mut dyn PlatformWindow,
    session: &mut WindowSession,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    clock: std::sync::Arc<dyn AppClock>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    metrics: Option<&Cell<RenderMetrics>>,
    map_event: M,
    on_exit: X,
    on_frame: F,
) -> i32
where
    M: Fn(&UiEvent) -> Option<SystemEvent>,
    X: Fn(&UiEvent) -> bool,
    F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn Platform),
{
    run_window_session_loop_with_system_theme_and_clock(
        platform,
        platform_window,
        session,
        font_service,
        image_service,
        theme,
        None,
        clock,
        debug_mode,
        cursor_pos,
        metrics,
        map_event,
        on_exit,
        |_| {},
        |_, _| {},
        || None,
        on_frame,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_window_session_loop_with_system_theme_and_clock<M, X, T, R, D, F>(
    platform: &mut dyn Platform,
    platform_window: &mut dyn PlatformWindow,
    session: &mut WindowSession,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    system_theme_tokens: Option<&DynTokens>,
    clock: std::sync::Arc<dyn AppClock>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    metrics: Option<&Cell<RenderMetrics>>,
    map_event: M,
    on_exit: X,
    on_runtime_tasks: T,
    on_foreign_event: R,
    next_external_deadline: D,
    on_frame: F,
) -> i32
where
    M: Fn(&UiEvent) -> Option<SystemEvent>,
    X: Fn(&UiEvent) -> bool,
    T: FnMut(&mut dyn Platform),
    R: FnMut(&UiEvent, &mut dyn Platform),
    D: FnMut() -> Option<Instant>,
    F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn Platform),
{
    let parts = session.parts_mut();
    run_widget_loop_with_active_work(
        platform,
        platform_window,
        parts.engine,
        parts.tree,
        parts.active_work,
        parts.app_timers,
        parts.main_thread_queue,
        clock,
        Some(parts.view_factory),
        parts.pending_root,
        parts.reconcile_pending,
        Some(parts.loop_state),
        font_service,
        image_service,
        theme,
        system_theme_tokens,
        debug_mode,
        cursor_pos,
        metrics,
        map_event,
        on_exit,
        on_runtime_tasks,
        on_foreign_event,
        next_external_deadline,
        on_frame,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_widget_loop_with_active_work<M, X, T, R, D, F>(
    platform: &mut dyn Platform,
    platform_window: &mut dyn PlatformWindow,
    engine: &mut dyn GraphicsEngine,
    tree: &mut WidgetTree,
    active_work: &mut ActiveWorkRegistry,
    app_timers: crate::app::app_timer::AppTimerQueue,
    main_thread_queue: crate::app::main_thread_queue::MainThreadQueue,
    clock: std::sync::Arc<dyn AppClock>,
    view_factory: Option<&crate::app::window_session::ViewFactorySlot>,
    pending_root: &mut Option<crate::ui::view::ViewNode>,
    reconcile_pending: &mut bool,
    mut loop_state: Option<&mut WindowLoopState>,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    system_theme_tokens: Option<&DynTokens>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    metrics: Option<&Cell<RenderMetrics>>,
    map_event: M,
    on_exit: X,
    mut on_runtime_tasks: T,
    mut on_foreign_event: R,
    mut next_external_deadline: D,
    on_frame: F,
) -> i32
where
    M: Fn(&UiEvent) -> Option<SystemEvent>,
    X: Fn(&UiEvent) -> bool,
    T: FnMut(&mut dyn Platform),
    R: FnMut(&UiEvent, &mut dyn Platform),
    D: FnMut() -> Option<Instant>,
    F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn Platform),
{
    let bus_ptr: *mut dyn Platform = platform as *mut dyn Platform;
    let window_id = platform_window.window_id();

    let pending_events = RefCell::new(Vec::<UiEvent>::new());
    let foreign_events = RefCell::new(Vec::<UiEvent>::new());
    tree.bind_invalidation();
    // bind 会清空失效队列；首帧须保留全帧 Paint，避免 RenderObject 重放空缓存
    tree.mark_full_frame_dirty();

    let mut first_frame = true;
    let mut rendered_first = false;
    let mut last_frame = clock.now();
    let mut window_visible = true;
    let mut frame_renderer = FrameRenderer::new();
    // debug overlay：仅当 hit 目标变化时全帧标脏（非每 move）。
    let last_debug_hover = Cell::new(None::<ComponentId>);
    let mut initial_size = (
        platform_window.properties().width(),
        platform_window.properties().height(),
    );

    {
        let c: &mut dyn crate::native::traits::input::IClipboard = platform.clipboard();
        let wide: *mut dyn crate::native::traits::input::IClipboard = c;
        let parts: (usize, usize) = unsafe { std::mem::transmute(wide) };
        clipboard::set_clipboard_parts(parts.0, parts.1);
    }

    let running = Cell::new(true);
    active_work.sync_app_timers(app_timers.deadlines());
    let mut ime_session = None;

    let collect = |ev: &UiEvent| {
        if ev.window_id.is_some_and(|target| target != window_id) {
            foreign_events.borrow_mut().push(ev.clone());
            return true;
        }
        match ev.type_ {
            UiEventType::WindowClose => {
                running.set(false);
                return false;
            }
            _ => {
                if on_exit(ev) {
                    running.set(false);
                    return false;
                }
            }
        }
        pending_events.borrow_mut().push(ev.clone());
        true
    };

    while running.get() {
        if first_frame {
            set_loop_state(&mut loop_state, WindowLoopState::Active);
            if !platform.event_loop().poll_event(&collect) {
                break;
            }
            first_frame = false;
        } else if !main_thread_queue.is_empty() || *reconcile_pending {
            set_loop_state(&mut loop_state, WindowLoopState::Active);
        } else {
            let external_deadline = next_external_deadline();
            set_loop_state(
                &mut loop_state,
                wait_loop_state(active_work, external_deadline),
            );
            if !wait_for_event_or_registered_work(
                platform,
                active_work,
                external_deadline,
                clock.as_ref(),
                &collect,
            ) {
                break;
            }
            platform.event_loop().poll_event(&collect);
        }

        for ev in foreign_events.borrow_mut().drain(..) {
            on_foreign_event(&ev, platform);
        }

        let had_events = !pending_events.borrow().is_empty();
        let mut had_layout_event = false;
        for ev in pending_events.borrow_mut().drain(..) {
            let is_layout_event = matches!(
                ev.type_,
                UiEventType::WindowResize
                    | UiEventType::WindowMaximize
                    | UiEventType::WindowRestore
            );
            if is_layout_event {
                had_layout_event = true;
            }

            match ev.type_ {
                UiEventType::WindowResize => {
                    if let UiEventPayload::Resize(ref d) = ev.payload {
                        if d.width > 0 && d.height > 0 {
                            engine.resize(d.width, d.height);
                            report_resize_notify_error(
                                "window resize_notify failed",
                                platform_window.resize_notify(d.width, d.height),
                            );
                            initial_size = (d.width, d.height);
                            tree.mark_full_frame_dirty();
                        }
                    }
                }
                UiEventType::WindowMaximize => {
                    if engine.canvas_2d().width() != initial_size.0
                        || engine.canvas_2d().height() != initial_size.1
                    {
                        // 已通过 resize 事件调整
                    } else {
                        let info = platform.display().info(0);
                        let w = info.bounds.w as i32;
                        let h = info.bounds.h as i32;
                        if w > 0 && h > 0 {
                            engine.resize(w, h);
                            report_resize_notify_error(
                                "window maximize resize_notify failed",
                                platform_window.resize_notify(w, h),
                            );
                            tree.mark_full_frame_dirty();
                        }
                    }
                }
                UiEventType::WindowRestore => {
                    let (rw, rh) = initial_size;
                    engine.resize(rw, rh);
                    report_resize_notify_error(
                        "window restore resize_notify failed",
                        platform_window.resize_notify(rw, rh),
                    );
                    window_visible = true;
                    tree.mark_full_frame_dirty();
                }
                UiEventType::WindowMinimize => {
                    window_visible = false;
                }
                UiEventType::PointerMove => {
                    if let UiEventPayload::PointerMove(ref data) = ev.payload {
                        cursor_pos.set(data.pos);
                        // debug hover 链：仅 hit 目标变化时标脏（#105；非每 move 全帧）。
                        if debug_mode.get() {
                            let hit = tree.hit_test(data.pos);
                            if hit != last_debug_hover.get() {
                                last_debug_hover.set(hit);
                                tree.mark_full_frame_dirty();
                            }
                        }
                    }
                }
                UiEventType::KeyDown => {
                    use crate::native::traits::input::{KeyCode, KeyMod};
                    if let UiEventPayload::Key(ref data) = ev.payload {
                        // Ctrl+Shift+D：切换 debug overlay。
                        // 不用 F12：Windows 调试器下 F12 会触发系统 DebugBreak（DbgBreakPoint/int3），
                        // 表现为「一按 F12 就停在异常」——与应用无关（MS KB Q130667）。
                        let toggle_debug = data.key == KeyCode::D
                            && data.mods.contains(KeyMod::CTRL)
                            && data.mods.contains(KeyMod::SHIFT);
                        if toggle_debug {
                            let next = !debug_mode.get();
                            debug_mode.set(next);
                            if !next {
                                last_debug_hover.set(None);
                            }
                            tree.mark_full_frame_dirty();
                            continue;
                        }
                    }
                }
                UiEventType::ThemeChanged => {
                    if let Some(tokens) = system_theme_tokens {
                        let is_dark = platform.display().is_dark_mode();
                        tokens.set_mode(is_dark);
                        tree.dispatch_event(&SystemEvent::ThemeChanged { is_dark });
                        let normalized_event = UiEvent::theme_changed(is_dark);
                        on_foreign_event(&normalized_event, platform);
                    }
                    unsafe {
                        (*bus_ptr).event_bus().publish(&ev);
                    }
                    continue;
                }
                _ => {}
            }

            sync_ime_session(tree, active_work, &mut ime_session, &ev, platform);
            if let Some(we) = map_event(&ev) {
                tree.dispatch_event(&we);
            }

            unsafe {
                (*bus_ptr).event_bus().publish(&ev);
            }
        }

        active_work.sync_timers(tree.active_timers(), clock.now());
        active_work.sync_app_timers(app_timers.deadlines());
        let now = clock.now();
        let due_work = active_work.drain_due(now);
        let had_registered_work = !due_work.is_empty();
        let due_animation_ids = due_animation_ids(&due_work);
        let had_due_widget_timer_work =
            dispatch_due_active_work(tree, &app_timers, &due_work, clock.as_ref());
        let mut main_thread_context = MainThreadContext::new(pending_root, reconcile_pending);
        let _had_main_thread_work = main_thread_queue.drain(&mut main_thread_context);
        let had_app_state_semantic_work = tree.drain_app_state_semantic_events();
        active_work.sync_timers(tree.active_timers(), clock.now());
        active_work.sync_app_timers(app_timers.deadlines());
        on_runtime_tasks(platform);
        if tree.take_reconcile_requested() {
            *reconcile_pending = true;
        }

        let now = clock.now();
        let dt = (now - last_frame).as_secs_f64().min(0.05);
        let pending_layout_work = tree
            .invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .has_layout();
        let pending_effects = tree.has_pending_effects();
        let pending_render_work = tree.has_render_work();
        let active_frame = had_events
            || had_registered_work
            || had_app_state_semantic_work
            || *reconcile_pending
            || pending_effects
            || !rendered_first
            || pending_layout_work
            || pending_render_work;
        let discover_animation_work = had_events
            || had_due_widget_timer_work
            || had_app_state_semantic_work
            || *reconcile_pending
            || pending_effects
            || !rendered_first
            || pending_layout_work
            || pending_render_work;
        if active_frame {
            last_frame = now;
            let animation_updates = update_due_and_discovered_animations(
                tree,
                active_work,
                &due_animation_ids,
                dt,
                discover_animation_work,
            );
            sync_animation_deadlines(active_work, &animation_updates, now);
            if pending_effects {
                let _effects_ran = tree.tick_effects();
            }
        }

        if tree.take_reconcile_requested() {
            *reconcile_pending = true;
        }

        if *reconcile_pending {
            let root = pending_root
                .take()
                .or_else(|| view_factory.and_then(|factory| factory.build()));
            if let Some(root) = root {
                crate::ui::view::ViewAdapter::reconcile_nodes(tree, root);
            }
            *reconcile_pending = false;
        }

        let has_layout_work = tree
            .invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .has_layout();

        let needs_work = window_visible && (had_layout_event || !rendered_first);

        if window_visible && (needs_work || has_layout_work) {
            let before_version = tree.tree_version();
            tree.layout();
            record_layout(metrics);

            sync_root_frame_to_engine(tree, engine);
            on_frame(tree, engine, platform);
            sync_root_frame_to_engine(tree, engine);

            if tree.tree_version() != before_version {
                tree.layout();
                record_layout(metrics);
                tree.mark_full_frame_dirty();
            }
        }

        let need_render = window_visible && (!rendered_first || tree.has_render_work());
        let engine_capabilities = engine.capabilities();

        if !rendered_first && need_render {
            tree.mark_full_frame_dirty();
            // 首帧绘制前强制 layout，确保 bind 后 frame 与引擎尺寸一致
            tree.layout();
        }

        let dirty_region = tree.dirty_region();

        let (outcome, outcome_source) = if !need_render {
            (RenderOutcome::Idle, InvalidationSource::None)
        } else {
            let theme_ref = theme.borrow();
            let snapshot = ThemeSnapshot::new(theme_ref.tokens());
            let scroll_move = tree.drain_scroll_region_move();
            let hover_pos = if debug_mode.get() {
                Some(cursor_pos.get())
            } else {
                None
            };
            let metrics_ref = metrics.map(|m| m.get());
            let frame_out = frame_renderer.render_frame(
                engine,
                tree,
                FrameRenderInput {
                    rendered_first,
                    dirty_region: &dirty_region,
                    tree_version: tree.tree_version(),
                    scroll_move,
                    theme: snapshot,
                    font: font_service.loaded_font_handle,
                    font_service,
                    image_service,
                    debug_mode: debug_mode.get(),
                    hover_pos,
                    metrics: metrics_ref.as_ref(),
                },
            );
            rendered_first = true;
            (frame_out.outcome, frame_out.inv_source)
        };

        if window_visible && (needs_work || has_layout_work || need_render) {
            // 每帧末尾清空已消费的失效队列，隐藏窗口保留 pending dirty 到恢复可见。
            tree.reset_invalidation();
        }

        match outcome {
            RenderOutcome::Present(damage) => {
                record_present(metrics, outcome_source);
                if engine_capabilities.uses_external_presenter() {
                    let canvas = engine.canvas_2d();
                    let cw = canvas.width();
                    let ch = canvas.height();
                    if let Err(e) = platform_window.presenter().present(
                        canvas.pixels_mut(),
                        cw,
                        ch,
                        damage.to_present_damage(),
                    ) {
                        crate::core::log::error_fn(format!(
                            "[EventLoop] present failed: {}",
                            e.short_what()
                        ));
                    }
                }
            }
            RenderOutcome::Idle => {
                record_idle(metrics, outcome_source);
            }
        }

        let next_state = next_loop_state(tree, active_work, next_external_deadline());
        set_loop_state(&mut loop_state, next_state);
    }

    0
}

fn record_layout(metrics: Option<&Cell<RenderMetrics>>) {
    if let Some(m) = metrics {
        let mut stats = m.get();
        stats.record_layout();
        m.set(stats);
    }
}

fn wait_for_event_or_registered_work(
    platform: &mut dyn Platform,
    active_work: &ActiveWorkRegistry,
    external_deadline: Option<Instant>,
    clock: &dyn AppClock,
    callback: &dyn Fn(&UiEvent) -> bool,
) -> bool {
    match earliest_deadline(active_work.next_deadline(), external_deadline) {
        Some(deadline) => {
            let now = clock.now();
            if deadline <= now {
                true
            } else {
                platform
                    .event_loop()
                    .wait_timeout(deadline.duration_since(now), callback)
            }
        }
        None => platform.event_loop().wait_event(callback),
    }
}

fn earliest_deadline(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(deadline), None) | (None, Some(deadline)) => Some(deadline),
        (None, None) => None,
    }
}

fn dispatch_due_active_work(
    tree: &mut WidgetTree,
    app_timers: &crate::app::app_timer::AppTimerQueue,
    due_work: &[crate::app::active_work_registry::ActiveWorkKind],
    clock: &dyn AppClock,
) -> bool {
    let mut handled_widget_timer = false;
    for work in due_work {
        match *work {
            crate::app::active_work_registry::ActiveWorkKind::Timer(id) => {
                handled_widget_timer |= tree.dispatch_timer_work(id) == EventResult::Handled;
            }
            crate::app::active_work_registry::ActiveWorkKind::AppTimer(id) => {
                app_timers.fire(id, clock.now());
            }
            _ => {}
        }
    }
    handled_widget_timer
}

fn due_animation_ids(due_work: &[ActiveWorkKind]) -> Vec<NodeId> {
    due_work
        .iter()
        .filter_map(|work| match *work {
            ActiveWorkKind::Animation(id) => Some(id),
            _ => None,
        })
        .collect()
}

fn update_due_and_discovered_animations(
    tree: &mut WidgetTree,
    active_work: &ActiveWorkRegistry,
    due_animation_ids: &[NodeId],
    dt: f64,
    discover_animation_work: bool,
) -> Vec<(NodeId, bool)> {
    let mut updates = if due_animation_ids.is_empty() {
        Vec::new()
    } else {
        tree.update_animation_nodes(due_animation_ids.iter().copied(), dt)
    };

    if discover_animation_work {
        let mut registered_ids: Vec<_> = active_work.animation_ids().collect();
        registered_ids.extend_from_slice(due_animation_ids);
        updates.extend(tree.update_animations_except(registered_ids, dt));
    }

    updates
}

fn sync_animation_deadlines(
    active_work: &mut ActiveWorkRegistry,
    animation_updates: &[(NodeId, bool)],
    now: std::time::Instant,
) {
    for &(id, animating) in animation_updates {
        let kind = ActiveWorkKind::Animation(id);
        if animating {
            active_work.register(kind, now + ANIMATION_FRAME_INTERVAL);
        } else {
            active_work.unregister(kind);
        }
    }
}

fn wait_loop_state(
    active_work: &ActiveWorkRegistry,
    external_deadline: Option<Instant>,
) -> WindowLoopState {
    if !active_work.is_empty() || external_deadline.is_some() {
        WindowLoopState::RegisteredActive
    } else {
        WindowLoopState::DeepIdle
    }
}

fn next_loop_state(
    tree: &WidgetTree,
    active_work: &ActiveWorkRegistry,
    external_deadline: Option<Instant>,
) -> WindowLoopState {
    if tree.has_render_work() || has_layout_work(tree) {
        WindowLoopState::Active
    } else {
        wait_loop_state(active_work, external_deadline)
    }
}

fn has_layout_work(tree: &WidgetTree) -> bool {
    tree.invalidation
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .has_layout()
}

fn sync_ime_session(
    tree: &WidgetTree,
    active_work: &mut ActiveWorkRegistry,
    current_session: &mut Option<NodeId>,
    ev: &UiEvent,
    platform: &mut dyn Platform,
) {
    match ev.type_ {
        UiEventType::ImeCompositionStart | UiEventType::ImeCompositionUpdate => {
            let Some(target) = tree.managers().focus.focused_component() else {
                return;
            };
            if *current_session != Some(target) {
                if let Some(previous) = *current_session {
                    active_work.unregister(ActiveWorkKind::ImeSession(previous));
                    if let Err(err) = platform.text_input().stop() {
                        crate::core::log::error_fn(format!(
                            "IME session stop failed: {}",
                            err.short_what()
                        ));
                    }
                }
                if let Err(err) = platform.text_input().start() {
                    crate::core::log::error_fn(format!(
                        "IME session start failed: {}",
                        err.short_what()
                    ));
                    return;
                }
                *current_session = Some(target);
            }
            active_work.register_open(ActiveWorkKind::ImeSession(target));
        }
        UiEventType::ImeCompositionEnd | UiEventType::WindowBlur => {
            if let Some(target) = current_session.take() {
                active_work.unregister(ActiveWorkKind::ImeSession(target));
                if let Err(err) = platform.text_input().stop() {
                    crate::core::log::error_fn(format!(
                        "IME session stop failed: {}",
                        err.short_what()
                    ));
                }
            }
        }
        _ => {}
    }
}

fn set_loop_state(state_slot: &mut Option<&mut WindowLoopState>, state: WindowLoopState) {
    if let Some(slot) = state_slot.as_deref_mut() {
        *slot = state;
    }
}

fn record_present(metrics: Option<&Cell<RenderMetrics>>, source: InvalidationSource) {
    if let Some(m) = metrics {
        let mut stats = m.get();
        stats.record_present(source);
        m.set(stats);
    }
}

fn record_idle(metrics: Option<&Cell<RenderMetrics>>, source: InvalidationSource) {
    if let Some(m) = metrics {
        let mut stats = m.get();
        stats.record_idle_with_source(source);
        m.set(stats);
    }
}

fn sync_root_frame_to_engine(tree: &mut WidgetTree, engine: &mut dyn GraphicsEngine) {
    let (ew, eh) = {
        let canvas = engine.canvas_2d();
        (canvas.width() as f32, canvas.height() as f32)
    };
    if ew <= 0.0 || eh <= 0.0 {
        return;
    }
    // 根已有有效客户区时以 SystemEvent::Resize / 会话初始尺寸为准，
    // 不得用可能滞后的引擎尺寸覆盖（否则放大/缩小后内容卡在旧几何）。
    // 仅 bootstrap（根仍近空）时从引擎补齐。
    let need_sync = tree
        .root_id()
        .and_then(|rid| tree.get(rid))
        .is_some_and(|root| {
            let rf = root.frame();
            rf.w <= 1.0 || rf.h <= 1.0
        });
    if need_sync {
        if let Some(rid) = tree.root_id() {
            if let Some(root_mut) = tree.get_mut(rid) {
                root_mut.set_frame(Rect::new(0.0, 0.0, ew, eh));
            }
        }
        tree.mark_full_frame_dirty();
        tree.layout();
    }
}

#[cfg(test)]
#[path = "../../tests/app/event_loop/event_loop.rs"]
mod tests;
