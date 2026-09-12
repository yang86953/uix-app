//! Render Loop — OS 事件 + Widget 调度；渲染段委托 draw ScenePipeline。

use crate::app::queues::active_work_registry::ActiveWorkRegistry;
use crate::app::queues::clock::{AppClock, earliest_deadline, system_clock};
use crate::app::queues::window_agent_state::WindowAgentState;
use crate::app::window::text_input::sync_window_text_input;
use crate::app::window::window_actions::apply_pending_window_actions;
use crate::app::window::window_driver::{WindowDriver, WindowFrameContext};
use crate::app::window::window_session::{WindowLoopState, WindowSession, WindowTextInputState};
use crate::app::window_semantics::WindowSemanticState;
use crate::core::Point;
use crate::diagnostics::Diagnostics;
use crate::draw::debug::DebugHudState;
use crate::draw::renderer::RenderMetrics;
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::image::ImageService;
use crate::draw::target::RenderTarget;
use crate::platform::platform::PlatformSystem;
use crate::platform::windowing::MouseButton;
use crate::platform::windowing::event::{UiEvent, UiEventPayload, UiEventType};
use crate::platform::windowing::window::PlatformWindow;
use crate::ui::theme::{ModeTokens, Theme};
use crate::ui::widget_runtime::clipboard;
use crate::ui::{SystemEvent, WidgetId, WidgetTree};
use std::cell::{Cell, RefCell};
use std::time::Instant;

/// Keeps only the latest adjacent resize for one routing target. Any other
/// event remains an ordering barrier.
pub(crate) fn push_coalesced_event(events: &mut Vec<UiEvent>, event: &UiEvent) {
    if event.type_ == UiEventType::WindowResize {
        if let Some(pending) = events.last_mut() {
            if pending.type_ == UiEventType::WindowResize && pending.window_id == event.window_id {
                pending.clone_from(event);
                return;
            }
        }
    }
    events.push(event.clone());
}

/// debug HUD 的指针拦截：把左键按下、移动与抬起翻译为 HUD 会话操作。
///
/// 返回是否被 HUD 消费；被消费的事件不再进入应用树分发，避免拖动或
/// 折叠 HUD 时误触下层组件。
fn debug_hud_consumes_pointer(hud: &DebugHudState, ev: &UiEvent) -> bool {
    match (&ev.type_, &ev.payload) {
        (UiEventType::PointerDown, UiEventPayload::PointerButton(data))
            if data.btn == MouseButton::Left =>
        {
            hud.pointer_down(data.pos)
        }
        (UiEventType::PointerMove, UiEventPayload::PointerMove(data)) => hud.pointer_move(data.pos),
        (UiEventType::PointerUp, UiEventPayload::PointerButton(data)) => hud.pointer_up(data.pos),
        _ => false,
    }
}

/// 运行完整的 widget 渲染事件循环。
// 保留旧的完整事件循环入口，供兼容组装方按需调用。
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_widget_loop<M, X, F>(
    platform: &mut dyn PlatformSystem,
    platform_window: &mut dyn PlatformWindow,
    engine: &mut dyn RenderTarget,
    tree: &mut WidgetTree,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Diagnostics,
    cursor_pos: &Cell<Point>,
    metrics: Option<&Cell<RenderMetrics>>,
    map_event: M,
    on_exit: X,
    on_frame: F,
) -> i32
where
    M: Fn(&UiEvent) -> Option<SystemEvent>,
    X: Fn(&UiEvent) -> bool,
    F: Fn(&mut WidgetTree, &mut dyn RenderTarget, &mut dyn PlatformSystem),
{
    let mut active_work = ActiveWorkRegistry::new();
    let mut pending_root = None;
    let mut reconcile_pending = false;
    let mut text_input = WindowTextInputState::default();
    let mut semantic_state = WindowSemanticState::new(platform_window.window_id());
    let mut agent_commands = WindowAgentState::new();
    let hud_state = DebugHudState::default();
    run_widget_loop_with_active_work(
        platform,
        platform_window,
        engine,
        tree,
        &mut active_work,
        crate::app::queues::app_timer::AppTimerQueue::new(),
        crate::app::queues::main_thread_queue::MainThreadQueue::new(),
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
        &hud_state,
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
    platform: &mut dyn PlatformSystem,
    platform_window: &mut dyn PlatformWindow,
    session: &mut WindowSession,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    system_theme_tokens: Option<&ModeTokens>,
    debug_mode: &Diagnostics,
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
    T: FnMut(&mut dyn PlatformSystem, &mut WidgetTree),
    R: FnMut(&UiEvent, &mut dyn PlatformSystem),
    D: FnMut() -> Option<Instant>,
    F: Fn(&mut WidgetTree, &mut dyn RenderTarget, &mut dyn PlatformSystem),
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
fn run_window_session_loop_with_system_theme_and_clock<M, X, T, R, D, F>(
    platform: &mut dyn PlatformSystem,
    platform_window: &mut dyn PlatformWindow,
    session: &mut WindowSession,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    system_theme_tokens: Option<&ModeTokens>,
    clock: std::sync::Arc<dyn AppClock>,
    debug_mode: &Diagnostics,
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
    T: FnMut(&mut dyn PlatformSystem, &mut WidgetTree),
    R: FnMut(&UiEvent, &mut dyn PlatformSystem),
    D: FnMut() -> Option<Instant>,
    F: Fn(&mut WidgetTree, &mut dyn RenderTarget, &mut dyn PlatformSystem),
{
    let parts = session.parts_mut();
    let hud_state = DebugHudState::default();
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
        &hud_state,
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
    platform: &mut dyn PlatformSystem,
    platform_window: &mut dyn PlatformWindow,
    engine: &mut dyn RenderTarget,
    tree: &mut WidgetTree,
    active_work: &mut ActiveWorkRegistry,
    app_timers: crate::app::queues::app_timer::AppTimerQueue,
    main_thread_queue: crate::app::queues::main_thread_queue::MainThreadQueue,
    agent_commands: &mut WindowAgentState,
    clock: std::sync::Arc<dyn AppClock>,
    view_factory: Option<&crate::app::window::window_session::ViewFactorySlot>,
    pending_root: &mut Option<crate::ui::view::ViewNode>,
    reconcile_pending: &mut bool,
    mut loop_state: Option<&mut WindowLoopState>,
    text_input: &mut WindowTextInputState,
    semantic_state: &mut WindowSemanticState,
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    system_theme_tokens: Option<&ModeTokens>,
    debug_mode: &Diagnostics,
    cursor_pos: &Cell<Point>,
    hud_state: &DebugHudState,
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
    T: FnMut(&mut dyn PlatformSystem, &mut WidgetTree),
    R: FnMut(&UiEvent, &mut dyn PlatformSystem),
    D: FnMut() -> Option<Instant>,
    F: Fn(&mut WidgetTree, &mut dyn RenderTarget, &mut dyn PlatformSystem),
{
    let bus_ptr: *mut dyn PlatformSystem = platform as *mut dyn PlatformSystem;
    // SAFETY: `bus_ptr` 由函数参数 `platform`（`&mut dyn PlatformSystem`，本函数作用域内
    // 存活）直接转换而来，event loop 运行期间指针始终有效。两处解引用均发生在
    // event loop 主线程，且解引用时不存在对 `platform` 的其他活跃可变借用
    // （`clipboard::with_clipboard` / `sync_window_text_input` 等借用点均已结束）；
    // `event_bus()` 返回的 `&mut EventBus` 只用于瞬时 `publish`，未逃逸。
    let window_id = platform_window.window_id();
    let native_window = platform_window.native_handle().native_window();

    let pending_events = RefCell::new(Vec::<UiEvent>::new());
    let foreign_events = RefCell::new(Vec::<UiEvent>::new());
    // 未知初始值保证每个窗口循环至少同步一次平台光标。
    let active_pointer_cursor = Cell::new(None);
    tree.bind_invalidation();
    // bind 会清空失效队列；首帧须保留全帧 Paint，避免 RenderObject 重放空缓存
    tree.mark_full_frame_dirty();

    let mut first_frame = true;
    let mut driver = WindowDriver::new(
        platform_window.properties().width(),
        platform_window.properties().height(),
        !platform_window.is_visible(),
        debug_mode.clone(),
    );
    // debug overlay：仅当 hit 目标变化时全帧标脏（非每 move）。
    let last_debug_hover = Cell::new(None::<WidgetId>);
    let mut fallback_loop_state = WindowLoopState::Active;

    let running = Cell::new(true);
    driver.sync_app_timers(active_work, &app_timers);

    let collect = |ev: &UiEvent| {
        if ev.window_id.is_some_and(|target| target != window_id) {
            push_coalesced_event(&mut foreign_events.borrow_mut(), ev);
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
        push_coalesced_event(&mut pending_events.borrow_mut(), ev);
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
                    // 把本窗口预算后保留的主线程任务纳入下一 deadline。
                    &main_thread_queue,
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

        let had_events = !pending_events.borrow().is_empty();
        // 同一批原生输入与其驱动的最终帧共享关联身份，便于从日志反查因果链。
        let mut debug_correlation_id =
            (debug_mode.debug_mode() && had_events).then(|| debug_mode.next_debug_correlation_id());
        let mut had_layout_event = false;
        for ev in pending_events.borrow_mut().drain(..) {
            if let Some(correlation_id) = debug_correlation_id {
                debug_mode.record_debug_input(
                    window_id,
                    correlation_id,
                    ev.type_.diagnostic_name(),
                );
                tracing::debug!(
                    target: "uix_app::diagnostics",
                    debug_event = "input_received",
                    correlation_id,
                    window_id = ?window_id,
                    event_type = ?ev.type_,
                    "window input entered the UI dispatch boundary"
                );
            }
            // debug HUD 优先消费落在其按钮/标题条或拖动会话上的指针事件，
            // 折叠与拖动不得同时触达下层应用组件。
            if debug_mode.debug_mode() && debug_hud_consumes_pointer(hud_state, &ev) {
                if let UiEventPayload::PointerMove(ref data) = ev.payload {
                    cursor_pos.set(data.pos);
                }
                tree.mark_full_frame_dirty();
                continue;
            }
            had_layout_event |=
                driver.handle_window_event(&ev, tree, engine, platform_window, text_input, debug_mode);

            match ev.type_ {
                UiEventType::PointerMove => {
                    if let UiEventPayload::PointerMove(ref data) = ev.payload {
                        cursor_pos.set(data.pos);
                        // 树事件处理已经确定悬停路由，此时同步有效继承光标。
                        super::pointer_cursor::apply_pointer_cursor(
                            // App System 持有平台能力根。
                            platform,
                            // UI System 只返回平台无关的有效光标值。
                            tree.active_pointer_cursor(),
                            // 相同请求由窗口循环状态去重。
                            &active_pointer_cursor,
                            debug_mode,
                        );
                        // debug hover 链：仅 hit 目标变化时标脏（#105；非每 move 全帧）。
                        if debug_mode.debug_mode() {
                            let hit = tree.hit_test(data.pos);
                            if hit != last_debug_hover.get() {
                                last_debug_hover.set(hit);
                                if let Some(correlation_id) = debug_correlation_id {
                                    debug_mode
                                        .record_debug_hover_changed(window_id, correlation_id);
                                    tracing::debug!(
                                        target: "uix_app::diagnostics",
                                        debug_event = "debug_hover_changed",
                                        correlation_id,
                                        window_id = ?window_id,
                                        widget_id = ?hit,
                                        pointer_x = data.pos.x,
                                        pointer_y = data.pos.y,
                                        "debug hover target changed"
                                    );
                                }
                                tree.mark_full_frame_dirty();
                            }
                        }
                    }
                }
                UiEventType::KeyDown => {
                    use crate::platform::windowing::{KeyCode, KeyMod};
                    if let UiEventPayload::Key(ref data) = ev.payload {
                        // Ctrl+Shift+D：切换 debug overlay。
                        // 不用 F12：Windows 调试器下 F12 会触发系统 DebugBreak（DbgBreakPoint/int3），
                        // 表现为「一按 F12 就停在异常」——与应用无关（MS KB Q130667）。
                        let toggle_debug = data.key == KeyCode::D
                            && data.mods.contains(KeyMod::CTRL)
                            && data.mods.contains(KeyMod::SHIFT);
                        if toggle_debug {
                            let next = !debug_mode.debug_mode();
                            debug_mode.set_debug_mode(next);
                            if next && debug_correlation_id.is_none() {
                                debug_correlation_id = Some(debug_mode.next_debug_correlation_id());
                            }
                            tracing::info!(
                                target: "uix_app::diagnostics",
                                debug_event = "shortcut_toggle",
                                correlation_id = debug_correlation_id.unwrap_or(0),
                                has_correlation = debug_correlation_id.is_some(),
                                window_id = ?window_id,
                                enabled = next,
                                "runtime debug mode toggled by keyboard"
                            );
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
                        let is_dark = platform.display().is_dark_mode().unwrap_or_else(|error| {
                            // 平台主题查询失败回退浅色：经冷却去重观察的自愈降级。
                            debug_mode.observe_transient_error(
                                "theme",
                                "theme query failed, defaulting to light", &error);
                            false
                        });
                        tokens.set_mode(is_dark);
                        *theme.borrow_mut() = tokens.snapshot();
                        clipboard::with_clipboard(platform.clipboard(), || {
                            tree.dispatch_event(&SystemEvent::ThemeChanged { is_dark });
                        });
                        let normalized_event = UiEvent::theme_changed(is_dark);
                        on_foreign_event(&normalized_event, platform);
                    }
                    unsafe {
                        // SAFETY: 见 `bus_ptr` 声明处注释——主线程、无重叠可变借用。
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
                // 当前原生事件的激活身份只在本轮同步动作消费期间有效。
                if let Err(error) = apply_pending_window_actions(
                    // 转交刚完成分发的窗口树。
                    tree,
                    // 命令只发往事件所属的原生窗口。
                    platform_window,
                    // 保留 PointerDown 与平台授权的精确因果关系。
                    ev.pointer_activation(),
                    // 结束当前事件动作执行参数。
                ) {
                    tracing::error!("window action failed: {}", error.short_what());
                    // 窗口动作失败终止主循环，终态事实进入框架报告。
                    debug_mode.report_with_origin(
                        error,
                        crate::diagnostics::ReportOrigin::framework("window", "action"),
                    );
                    running.set(false);
                    break;
                }
            }
            sync_window_text_input(
                tree,
                active_work,
                text_input,
                window_id,
                native_window,
                platform,
                debug_mode,
            );

            unsafe {
                // SAFETY: 见 `bus_ptr` 声明处注释——主线程、无重叠可变借用。
                (*bus_ptr).event_bus().publish(&ev);
            }
        }
        if !running.get() {
            break;
        }

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
            debug_correlation_id,
            hud_state: Some(hud_state),
            cursor_pos,
            metrics,
            now,
            had_events,
            had_layout_event,
            next_external_deadline: external_deadline,
            on_runtime_tasks: &mut on_runtime_tasks,
            on_frame: &on_frame,
        });
        // 协调可能在指针静止时替换悬停节点的 cursor 声明，帧后再次去重同步。
        super::pointer_cursor::apply_pointer_cursor(
            // App System 仍是唯一的平台调用所有者。
            platform,
            // 读取协调完成后的有效继承光标。
            tree.active_pointer_cursor(),
            // 已在事件路径应用的相同值不会重复调用平台。
            &active_pointer_cursor,
            debug_mode,
        );
        // 非原生事件触发的运行时动作不得借用历史指针授权。
        if let Err(error) = apply_pending_window_actions(
            // 转交当前窗口树。
            tree,
            // 转交当前原生窗口。
            platform_window,
            // 明确声明本轮没有原生输入激活上下文。
            None,
            // 结束运行时动作执行参数。
        ) {
            tracing::error!(
                "window action failed after runtime work: {}",
                error.short_what()
            );
            // 运行时工作后的窗口动作失败同样终止主循环，进入框架报告。
            debug_mode.report_with_origin(
                error,
                crate::diagnostics::ReportOrigin::framework("window", "action_after_work"),
            );
            break;
        }
    }

    text_input.window_focused = false;
    sync_window_text_input(
        tree,
        active_work,
        text_input,
        window_id,
        native_window,
        platform,
        debug_mode,
    );
    0
}

fn wait_for_event_or_registered_work(
    platform: &mut dyn PlatformSystem,
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

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../tests-src/app/event_loop/event_loop_tests.rs"]
mod event_loop_tests;