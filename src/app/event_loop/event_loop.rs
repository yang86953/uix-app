//! Render Loop — OS 事件 + Widget 调度；渲染段委托 draw FrameRenderer。

use crate::app::active_work_registry::ActiveWorkRegistry;
use crate::app::agent_control::WindowAgentState;
use crate::app::clock::{system_clock, AppClock};
use crate::app::text_input::sync_window_text_input;
use crate::app::window_driver::{WindowDriver, WindowFrameContext};
use crate::app::window_semantics::WindowSemanticState;
use crate::app::window_session::{WindowLoopState, WindowSession, WindowTextInputState};
use crate::core::Point;
use crate::draw::font::font_service::FontService;
use crate::draw::image::ImageService;
use crate::draw::pipeline::RenderMetrics;
use crate::draw::traits::GraphicsEngine;
use crate::native::traits::event::{UiEvent, UiEventPayload, UiEventType};
use crate::native::traits::platform::Platform;
use crate::native::traits::window::PlatformWindow;
use crate::ui::clipboard;
use crate::ui::theme::{DynTokens, Theme};
use crate::ui::{ComponentId, SystemEvent, WidgetTree};
use std::cell::{Cell, RefCell};
use std::time::Instant;

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
    let mut text_input = WindowTextInputState::default();
    let mut semantic_state = WindowSemanticState::new(platform_window.window_id());
    let mut agent_commands = WindowAgentState::new();
    run_widget_loop_with_active_work(
        platform,
        platform_window,
        engine,
        tree,
        &mut active_work,
        crate::app::app_timer::AppTimerQueue::new(),
        crate::app::main_thread_queue::MainThreadQueue::new(),
        &mut agent_commands,
        system_clock(),
        None,
        &mut pending_root,
        &mut reconcile_pending,
        None,
        &mut text_input,
        &mut semantic_state,
        font_service,
        image_service,
        theme,
        None,
        debug_mode,
        cursor_pos,
        metrics,
        map_event,
        on_exit,
        |_, _| {},
        |_, _| {},
        || None,
        on_frame,
    )
}

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
        |_, _| {},
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
    T: FnMut(&mut dyn Platform, &mut WidgetTree),
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
        |_, _| {},
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
    T: FnMut(&mut dyn Platform, &mut WidgetTree),
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
        parts.agent_commands,
        clock,
        Some(parts.view_factory),
        parts.pending_root,
        parts.reconcile_pending,
        Some(parts.loop_state),
        parts.text_input,
        parts.semantic_state,
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
    agent_commands: &mut WindowAgentState,
    clock: std::sync::Arc<dyn AppClock>,
    view_factory: Option<&crate::app::window_session::ViewFactorySlot>,
    pending_root: &mut Option<crate::ui::view::ViewNode>,
    reconcile_pending: &mut bool,
    mut loop_state: Option<&mut WindowLoopState>,
    text_input: &mut WindowTextInputState,
    semantic_state: &mut WindowSemanticState,
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
    T: FnMut(&mut dyn Platform, &mut WidgetTree),
    R: FnMut(&UiEvent, &mut dyn Platform),
    D: FnMut() -> Option<Instant>,
    F: Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn Platform),
{
    let bus_ptr: *mut dyn Platform = platform as *mut dyn Platform;
    let window_id = platform_window.window_id();
    let native_window = platform_window.native_handle().native_window();

    let pending_events = RefCell::new(Vec::<UiEvent>::new());
    let foreign_events = RefCell::new(Vec::<UiEvent>::new());
    tree.bind_invalidation();
    // bind 会清空失效队列；首帧须保留全帧 Paint，避免 RenderObject 重放空缓存
    tree.mark_full_frame_dirty();

    let mut first_frame = true;
    let mut driver = WindowDriver::new(
        platform_window.properties().width(),
        platform_window.properties().height(),
        !platform_window.is_visible(),
    );
    // debug overlay：仅当 hit 目标变化时全帧标脏（非每 move）。
    let last_debug_hover = Cell::new(None::<ComponentId>);
    let mut fallback_loop_state = WindowLoopState::Active;

    let running = Cell::new(true);
    active_work.sync_app_timers(app_timers.deadlines());

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
        } else {
            let now = clock.now();
            if driver.has_frame_work(
                now,
                tree,
                active_work,
                &app_timers,
                &main_thread_queue,
                agent_commands,
                pending_root,
                reconcile_pending,
            ) {
                set_loop_state(&mut loop_state, WindowLoopState::Active);
                if !platform.event_loop().poll_event(&collect) {
                    break;
                }
            } else {
                let window_deadline = driver.next_deadline(
                    now,
                    tree,
                    active_work,
                    &app_timers,
                    agent_commands,
                    pending_root,
                    *reconcile_pending,
                );
                let external_deadline =
                    earliest_deadline(window_deadline, next_external_deadline());
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
                if !platform.event_loop().poll_event(&collect) {
                    break;
                }
            }
        }

        for ev in foreign_events.borrow_mut().drain(..) {
            on_foreign_event(&ev, platform);
        }

        let input_t0 = Instant::now();
        let had_events = !pending_events.borrow().is_empty();
        let mut had_layout_event = false;
        for ev in pending_events.borrow_mut().drain(..) {
            had_layout_event |= driver.handle_window_event(
                &ev,
                tree,
                engine,
                platform_window,
                platform,
                text_input,
            );

            match ev.type_ {
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
                        *theme.borrow_mut() = Theme::new(tokens.snapshot());
                        clipboard::with_clipboard(platform.clipboard(), || {
                            tree.dispatch_event(&SystemEvent::ThemeChanged { is_dark });
                        });
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

            if let Some(we) = map_event(&ev) {
                clipboard::with_clipboard(platform.clipboard(), || {
                    tree.dispatch_event(&we);
                });
            }
            sync_window_text_input(
                tree,
                active_work,
                text_input,
                window_id,
                native_window,
                platform,
            );

            unsafe {
                (*bus_ptr).event_bus().publish(&ev);
            }
        }
        let phase_input_us = input_t0.elapsed().as_micros();

        let now = clock.now();
        let external_deadline = next_external_deadline();
        let loop_state_slot = loop_state
            .as_deref_mut()
            .unwrap_or(&mut fallback_loop_state);
        driver.drive_frame(WindowFrameContext {
            tree,
            engine,
            active_work,
            app_timers: &app_timers,
            main_thread_queue: &main_thread_queue,
            agent_commands,
            view_factory,
            pending_root,
            reconcile_pending,
            loop_state: loop_state_slot,
            text_input,
            semantic_state,
            platform_window,
            platform: Some(platform),
            font_service,
            image_service,
            theme,
            debug_mode,
            cursor_pos,
            metrics,
            now,
            had_events,
            had_layout_event,
            input_us: phase_input_us,
            next_external_deadline: external_deadline,
            on_runtime_tasks: &mut on_runtime_tasks,
            on_frame: &on_frame,
        });
    }

    text_input.window_focused = false;
    sync_window_text_input(
        tree,
        active_work,
        text_input,
        window_id,
        native_window,
        platform,
    );
    0
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

pub(crate) fn earliest_deadline(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(deadline), None) | (None, Some(deadline)) => Some(deadline),
        (None, None) => None,
    }
}

pub(crate) fn wait_loop_state(
    active_work: &ActiveWorkRegistry,
    external_deadline: Option<Instant>,
) -> WindowLoopState {
    if !active_work.is_empty() || external_deadline.is_some() {
        WindowLoopState::RegisteredActive
    } else {
        WindowLoopState::DeepIdle
    }
}

fn set_loop_state(state_slot: &mut Option<&mut WindowLoopState>, state: WindowLoopState) {
    if let Some(slot) = state_slot.as_deref_mut() {
        *slot = state;
    }
}
