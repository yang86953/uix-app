//! Shared per-window frame driver.
//!
//! The outer application loops own event collection and window lifetime. This
//! type owns the ordered work -> animation -> reconcile -> layout -> paint ->
//! present pipeline so root and secondary windows cannot drift into different
//! frame semantics.

mod availability;
mod support;

use crate::app::active_work_registry::{ActiveWorkKind, ActiveWorkRegistry, TimerId};
use crate::app::agent_control::WindowAgentState;
use crate::app::app_timer::AppTimerQueue;
use crate::app::frame_scheduler::{FrameScheduler, SurfaceSuspendReason};
use crate::app::main_thread_queue::{MainThreadContext, MainThreadQueue};
use crate::app::text_input::sync_window_text_input;
use crate::app::window_semantics::WindowSemanticState;
use crate::app::window_session::{ViewFactorySlot, WindowLoopState, WindowTextInputState};
use crate::core::{Errc, Error, Point, PresentDamageTracker, Rect};
use crate::draw::engine::GraphicsFailure;
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
use crate::native::traits::present::PresentTestResult;
use crate::native::traits::window::{NativeFrameRequest, PlatformWindow, WindowOcclusionState};
use crate::ui::clipboard;
use crate::ui::core::widget::WidgetCore;
use crate::ui::theme::Theme;
use crate::ui::view::ViewAdapter;
use crate::ui::{EventResult, WidgetTree};
use std::cell::{Cell, RefCell};
use std::time::Instant;
pub(crate) use support::{
    animation_clock_should_advance, ensure_surface_matches_window, has_invalidation_work,
    sync_root_frame_exactly_to_engine, sync_root_frame_to_engine, WindowFrameResult,
};
use support::{
    dispatch_due_active_work, earliest_deadline, has_layout_work, log_frame_metrics,
    next_loop_state, observe_agent_settle, protocol_failure, record_idle, record_layout,
    record_present, report_graphics_frame_failure, report_graphics_resize_error,
    report_window_operation_error, update_scheduled_and_discovered_animations,
    with_platform_clipboard,
};
#[cfg(test)]
pub(crate) use support::{graphics_failure_diagnostic, graphics_failure_is_error};

pub(crate) struct WindowFrameContext<'a, 'platform> {
    pub(crate) tree: &'a mut WidgetTree,
    pub(crate) engine: &'a mut dyn GraphicsEngine,
    pub(crate) active_work: &'a mut ActiveWorkRegistry,
    pub(crate) app_timers: &'a AppTimerQueue,
    pub(crate) main_thread_queue: &'a MainThreadQueue,
    pub(crate) agent_commands: &'a mut WindowAgentState,
    pub(crate) view_factory: Option<&'a ViewFactorySlot>,
    pub(crate) pending_root: &'a mut Option<crate::ui::view::ViewNode>,
    pub(crate) reconcile_pending: &'a mut bool,
    pub(crate) loop_state: &'a mut WindowLoopState,
    pub(crate) text_input: &'a mut WindowTextInputState,
    pub(crate) semantic_state: &'a mut WindowSemanticState,
    pub(crate) platform_window: &'a mut dyn PlatformWindow,
    pub(crate) platform: Option<&'platform mut dyn Platform>,
    pub(crate) font_service: &'a FontService,
    pub(crate) image_service: &'a ImageService,
    pub(crate) theme: &'a RefCell<Theme>,
    pub(crate) debug_mode: &'a Cell<bool>,
    pub(crate) cursor_pos: &'a Cell<Point>,
    pub(crate) metrics: Option<&'a Cell<RenderMetrics>>,
    pub(crate) now: Instant,
    pub(crate) had_events: bool,
    pub(crate) had_layout_event: bool,
    pub(crate) input_us: u128,
    pub(crate) next_external_deadline: Option<Instant>,
    pub(crate) on_runtime_tasks: &'a mut dyn FnMut(&mut dyn Platform, &mut WidgetTree),
    pub(crate) on_frame: &'a dyn Fn(&mut WidgetTree, &mut dyn GraphicsEngine, &mut dyn Platform),
}

pub(crate) struct WindowDriver {
    frame_renderer: FrameRenderer,
    rendered_first: bool,
    present_damage_tracker: PresentDamageTracker,
    frame_scheduler: FrameScheduler,
    last_frame: Option<Instant>,
    initial_size: (i32, i32),
    deferred_show: bool,
    started_at: Option<Instant>,
    scheduled_animation_ids_scratch: Vec<NodeId>,
    app_timer_deadlines_scratch: Vec<(TimerId, Instant)>,
}

impl WindowDriver {
    pub(crate) fn new(width: i32, height: i32, deferred_show: bool) -> Self {
        Self {
            frame_renderer: FrameRenderer::new(),
            rendered_first: false,
            present_damage_tracker: PresentDamageTracker::new(),
            frame_scheduler: FrameScheduler::new(width > 0 && height > 0),
            last_frame: None,
            initial_size: (width, height),
            deferred_show,
            started_at: None,
            scheduled_animation_ids_scratch: Vec::new(),
            app_timer_deadlines_scratch: Vec::new(),
        }
    }

    pub(crate) fn last_frame(&self) -> Option<Instant> {
        self.last_frame
    }

    pub(crate) fn sync_app_timers(
        &mut self,
        active_work: &mut ActiveWorkRegistry,
        app_timers: &AppTimerQueue,
    ) {
        app_timers.deadlines_into(&mut self.app_timer_deadlines_scratch);
        active_work.sync_app_timers(self.app_timer_deadlines_scratch.iter().copied());
    }

    /// Applies the shared native window lifecycle portion of an event.
    ///
    /// Returns whether the event is a layout-affecting window event. Semantic
    /// dispatch remains in the outer loop so its event mapper stays injectable.
    pub(crate) fn handle_window_event(
        &mut self,
        event: &UiEvent,
        tree: &mut WidgetTree,
        engine: &mut dyn GraphicsEngine,
        platform_window: &mut dyn PlatformWindow,
        platform: &mut dyn Platform,
        text_input: &mut WindowTextInputState,
    ) -> bool {
        match event.type_ {
            UiEventType::FrameOpportunity => {
                if let UiEventPayload::FrameOpportunity(ref data) = event.payload {
                    self.frame_scheduler.notify_opportunity(
                        data.token,
                        data.frame_time,
                        data.target_present_time,
                    );
                }
                false
            }
            UiEventType::WindowResize => {
                if let UiEventPayload::Resize(ref data) = event.payload {
                    self.cancel_outstanding_native_frame(platform_window);
                    if data.width > 0 && data.height > 0 {
                        let resized = report_graphics_resize_error(
                            "window graphics resize failed",
                            engine.resize(data.width, data.height),
                        );
                        report_window_operation_error(
                            "window resize_notify failed",
                            platform_window.resize_notify(data.width, data.height),
                        );
                        if resized {
                            sync_root_frame_exactly_to_engine(tree, engine);
                        }
                        self.initial_size = (data.width, data.height);
                        self.frame_scheduler.surface_changed();
                        tree.mark_full_frame_dirty();
                    } else {
                        self.frame_scheduler
                            .suspend(SurfaceSuspendReason::ZeroExtent);
                        tree.mark_full_frame_dirty();
                    }
                }
                true
            }
            UiEventType::WindowMaximize => {
                self.cancel_outstanding_native_frame(platform_window);
                self.frame_scheduler.resume();
                if engine.canvas_2d().width() == self.initial_size.0
                    && engine.canvas_2d().height() == self.initial_size.1
                {
                    let info = platform.display().info(0);
                    let width = info.bounds.w as i32;
                    let height = info.bounds.h as i32;
                    if width > 0 && height > 0 {
                        if report_graphics_resize_error(
                            "window graphics maximize resize failed",
                            engine.resize(width, height),
                        ) {
                            self.frame_scheduler.surface_changed();
                        }
                        report_window_operation_error(
                            "window maximize resize_notify failed",
                            platform_window.resize_notify(width, height),
                        );
                    }
                }
                tree.mark_full_frame_dirty();
                true
            }
            UiEventType::WindowRestore => {
                self.cancel_outstanding_native_frame(platform_window);
                let (width, height) = self.initial_size;
                report_graphics_resize_error(
                    "window graphics restore resize failed",
                    engine.resize(width, height),
                );
                report_window_operation_error(
                    "window restore resize_notify failed",
                    platform_window.resize_notify(width, height),
                );
                self.frame_scheduler.surface_changed();
                tree.mark_full_frame_dirty();
                true
            }
            UiEventType::WindowMinimize => {
                self.cancel_outstanding_native_frame(platform_window);
                self.frame_scheduler
                    .suspend(SurfaceSuspendReason::Minimized);
                false
            }
            UiEventType::WindowHide => {
                if self.deferred_show {
                    return false;
                }
                self.cancel_outstanding_native_frame(platform_window);
                if self.frame_scheduler.suspended_reason()
                    != Some(SurfaceSuspendReason::TerminalFailure)
                {
                    self.frame_scheduler.suspend(SurfaceSuspendReason::Hidden);
                }
                tree.mark_full_frame_dirty();
                false
            }
            UiEventType::WindowShow => {
                if self.frame_scheduler.suspended_reason() == Some(SurfaceSuspendReason::Hidden) {
                    self.frame_scheduler.resume();
                    tree.mark_full_frame_dirty();
                    true
                } else {
                    false
                }
            }
            // The notification is only a wake signal. The current native
            // occlusion property is queried immediately before visual work so
            // queued or coalesced notifications cannot apply stale state.
            UiEventType::WindowOcclusionChanged => false,
            UiEventType::WindowFocus => {
                text_input.window_focused = true;
                false
            }
            UiEventType::WindowBlur => {
                text_input.window_focused = false;
                false
            }
            _ => false,
        }
    }

    /// Synchronizes programmatic visibility changes that occur during the
    /// current work turn, before layout/paint/present can run. The initial
    /// intentionally hidden window is exempt until deferred first-show.
    fn suspend_if_surface_unavailable(
        &mut self,
        tree: &mut WidgetTree,
        platform_window: &mut dyn PlatformWindow,
    ) -> bool {
        if self.deferred_show {
            return false;
        }
        if !platform_window.is_visible() {
            match self.frame_scheduler.suspended_reason() {
                Some(SurfaceSuspendReason::Hidden)
                | Some(SurfaceSuspendReason::TerminalFailure) => {}
                _ => {
                    self.cancel_outstanding_native_frame(platform_window);
                    self.frame_scheduler.suspend(SurfaceSuspendReason::Hidden);
                    tree.mark_full_frame_dirty();
                }
            }
            return true;
        }

        let suspended_reason = self.frame_scheduler.suspended_reason();
        match platform_window.occlusion_state() {
            WindowOcclusionState::Occluded => {
                match suspended_reason {
                    Some(SurfaceSuspendReason::Occluded)
                    | Some(SurfaceSuspendReason::Minimized)
                    | Some(SurfaceSuspendReason::ZeroExtent)
                    | Some(SurfaceSuspendReason::TerminalFailure) => {}
                    _ => {
                        self.cancel_outstanding_native_frame(platform_window);
                        // A native lifecycle signal will wake this window on
                        // exposure, so this path deliberately registers no
                        // DXGI-style availability probe deadline.
                        self.frame_scheduler.suspend(SurfaceSuspendReason::Occluded);
                        tree.mark_full_frame_dirty();
                    }
                }
                return true;
            }
            WindowOcclusionState::Visible => {
                if matches!(
                    suspended_reason,
                    Some(SurfaceSuspendReason::Hidden | SurfaceSuspendReason::Occluded)
                ) {
                    self.frame_scheduler.resume();
                    tree.mark_full_frame_dirty();
                }
            }
            WindowOcclusionState::Unknown => {
                // Unsupported backends must not accidentally resume a typed
                // graphics occlusion that still requires its own exit probe.
                if suspended_reason == Some(SurfaceSuspendReason::Hidden) {
                    self.frame_scheduler.resume();
                    tree.mark_full_frame_dirty();
                }
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn has_frame_work(
        &mut self,
        now: Instant,
        tree: &mut WidgetTree,
        active_work: &mut ActiveWorkRegistry,
        app_timers: &AppTimerQueue,
        main_thread_queue: &MainThreadQueue,
        agent_commands: &WindowAgentState,
        pending_root: &Option<crate::ui::view::ViewNode>,
        reconcile_pending: &mut bool,
    ) -> bool {
        self.sync_app_timers(active_work, app_timers);
        if tree.take_reconcile_requested() {
            *reconcile_pending = true;
        }

        self.arm_visual_request(now, tree, pending_root, *reconcile_pending);

        let due_registered_work = active_work
            .next_deadline()
            .is_some_and(|deadline| deadline <= now);
        due_registered_work
            || !main_thread_queue.is_empty()
            || agent_commands.has_work()
            || tree.has_app_state_focus_requests()
            || tree.has_app_state_semantic_events()
            || tree.has_pending_effects()
            || self.frame_scheduler.has_due_occlusion_probe(now)
            || self.frame_scheduler.has_due_opportunity(now)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn next_deadline(
        &mut self,
        now: Instant,
        tree: &mut WidgetTree,
        active_work: &mut ActiveWorkRegistry,
        app_timers: &AppTimerQueue,
        agent_commands: &WindowAgentState,
        pending_root: &Option<crate::ui::view::ViewNode>,
        reconcile_pending: bool,
    ) -> Option<Instant> {
        self.sync_app_timers(active_work, app_timers);
        if tree.take_reconcile_requested() {
            self.arm_visual_request(now, tree, pending_root, true);
        } else {
            self.arm_visual_request(now, tree, pending_root, reconcile_pending);
        }
        earliest_deadline(
            agent_commands.has_work().then_some(now),
            earliest_deadline(
                active_work.next_deadline(),
                self.frame_scheduler.next_deadline(),
            ),
        )
    }

    fn arm_visual_request(
        &mut self,
        now: Instant,
        tree: &WidgetTree,
        pending_root: &Option<crate::ui::view::ViewNode>,
        reconcile_pending: bool,
    ) {
        if !self.frame_scheduler.is_renderable() {
            return;
        }
        if !self.rendered_first
            || pending_root.is_some()
            || reconcile_pending
            || has_invalidation_work(tree)
        {
            self.frame_scheduler.request_immediate(now);
        }
    }

    pub(crate) fn drive_frame(&mut self, context: WindowFrameContext<'_, '_>) -> WindowFrameResult {
        let WindowFrameContext {
            tree,
            engine,
            active_work,
            app_timers,
            main_thread_queue,
            agent_commands,
            view_factory,
            pending_root,
            reconcile_pending,
            loop_state,
            text_input,
            semantic_state,
            platform_window,
            mut platform,
            font_service,
            image_service,
            theme,
            debug_mode,
            cursor_pos,
            metrics,
            now,
            had_events,
            had_layout_event,
            input_us,
            next_external_deadline,
            on_runtime_tasks,
            on_frame,
        } = context;

        let frame_t0 = Instant::now();
        self.started_at.get_or_insert(now);
        self.publish_agent_window_availability(semantic_state, platform_window);

        active_work.sync_timers(tree.active_timers(), now);
        self.sync_app_timers(active_work, app_timers);
        let due_work = active_work.drain_due(now);
        let had_registered_work = !due_work.is_empty();
        if due_work.contains(&ActiveWorkKind::GraphicsMaintenance) {
            engine.release_idle_resources(now);
            sync_graphics_maintenance(active_work, engine);
        }
        let had_due_animation_work = due_work
            .iter()
            .any(|work| matches!(work, ActiveWorkKind::Animation(_)));
        let had_due_widget_timer_work = with_platform_clipboard(&mut platform, || {
            dispatch_due_active_work(tree, app_timers, &due_work, now)
        });

        let mut main_thread_context = MainThreadContext::new(pending_root, reconcile_pending);
        let had_main_thread_work = main_thread_queue.drain(&mut main_thread_context);
        let had_agent_pending = agent_commands.has_work();
        let had_agent_command_work = agent_commands.drain_ready(
            tree,
            semantic_state,
            self.agent_surface_presentable(platform_window),
        );
        let had_app_state_focus_work =
            with_platform_clipboard(&mut platform, || tree.drain_app_state_focus_requests());
        let had_app_state_semantic_work =
            with_platform_clipboard(&mut platform, || tree.drain_app_state_semantic_events());
        active_work.sync_timers(tree.active_timers(), now);
        self.sync_app_timers(active_work, app_timers);
        if let Some(platform) = platform.as_deref_mut() {
            on_runtime_tasks(platform, tree);
        }
        if tree.take_reconcile_requested() {
            *reconcile_pending = true;
        }

        let pending_effects = tree.has_pending_effects();
        if pending_effects {
            let _effects_ran = with_platform_clipboard(&mut platform, || tree.tick_effects());
            active_work.sync_timers(tree.active_timers(), now);
            self.sync_app_timers(active_work, app_timers);
        }
        if tree.take_reconcile_requested() {
            *reconcile_pending = true;
        }

        let event_work = had_events
            || had_registered_work
            || had_due_widget_timer_work
            || had_main_thread_work
            || had_agent_pending
            || had_agent_command_work
            || had_app_state_focus_work
            || had_app_state_semantic_work
            || pending_effects;

        if self.suspend_if_surface_unavailable(tree, platform_window) {
            active_work.park_animated_deadlines();
            agent_commands.fail_not_presentable();
            *loop_state = next_loop_state(
                tree,
                active_work,
                next_external_deadline,
                false,
                false,
                agent_commands.has_work(),
            );
            self.publish_agent_window_availability(semantic_state, platform_window);
            return WindowFrameResult {
                did_work: event_work,
            };
        }

        if agent_commands.has_in_flight() && !self.agent_surface_presentable(platform_window) {
            agent_commands.fail_not_presentable();
        }

        if self.frame_scheduler.take_due_occlusion_probe(now) {
            match engine.test_present() {
                Ok(PresentTestResult::Presentable) => {
                    self.frame_scheduler.resume();
                    tree.mark_full_frame_dirty();
                }
                Ok(PresentTestResult::Occluded) => {
                    self.frame_scheduler.occlusion_still_present(now);
                    active_work.park_animated_deadlines();
                    let registered_deadline = earliest_deadline(
                        self.frame_scheduler.next_deadline(),
                        next_external_deadline,
                    );
                    *loop_state = next_loop_state(
                        tree,
                        active_work,
                        registered_deadline,
                        false,
                        false,
                        agent_commands.has_work(),
                    );
                    self.publish_agent_window_availability(semantic_state, platform_window);
                    return WindowFrameResult { did_work: true };
                }
                Err(error) => {
                    crate::core::log::error_fn(format!(
                        "[WindowDriver] occlusion present test failed: {}",
                        error.short_what()
                    ));
                    let failure = GraphicsFailure::from_error(error);
                    self.frame_scheduler.frame_failed(&failure, now);
                    active_work.park_animated_deadlines();
                    let registered_deadline = earliest_deadline(
                        self.frame_scheduler.next_deadline(),
                        next_external_deadline,
                    );
                    *loop_state = next_loop_state(
                        tree,
                        active_work,
                        registered_deadline,
                        false,
                        false,
                        agent_commands.has_work(),
                    );
                    self.publish_agent_window_availability(semantic_state, platform_window);
                    return WindowFrameResult { did_work: true };
                }
            }
        }

        if had_due_animation_work {
            self.frame_scheduler.request_immediate(now);
        }
        self.arm_visual_request(now, tree, pending_root, *reconcile_pending);
        let Some(opportunity) = self.frame_scheduler.take_due_opportunity(now) else {
            observe_agent_settle(
                agent_commands,
                semantic_state,
                tree,
                main_thread_queue,
                pending_root,
                *reconcile_pending,
                has_layout_work(tree),
                true,
            );
            if !self.frame_scheduler.is_renderable() {
                active_work.park_animated_deadlines();
            }
            let registered_deadline =
                earliest_deadline(self.frame_scheduler.next_deadline(), next_external_deadline);
            *loop_state = next_loop_state(
                tree,
                active_work,
                registered_deadline,
                self.frame_scheduler.is_renderable(),
                false,
                agent_commands.has_work(),
            );
            self.publish_agent_window_availability(semantic_state, platform_window);
            return WindowFrameResult {
                did_work: event_work,
            };
        };

        if let Some(token) = opportunity.fallback_token() {
            if let Err(error) = platform_window.cancel_native_frame(token) {
                crate::core::log::warn_fn(format!(
                    "[WindowDriver] fallback native frame cancellation failed: {}",
                    error.short_what()
                ));
            }
        }

        let frame_time = opportunity.frame_time();
        let target_present_time = opportunity.target_present_time();
        self.last_frame = Some(frame_time);

        self.scheduled_animation_ids_scratch.clear();
        self.scheduled_animation_ids_scratch
            .extend(active_work.animation_ids());
        let scheduled_animation_ids = self.scheduled_animation_ids_scratch.as_slice();
        let discover_animation_work =
            event_work || !self.rendered_first || *reconcile_pending || has_invalidation_work(tree);
        let dt = self
            .frame_scheduler
            .animation_delta(frame_time, !scheduled_animation_ids.is_empty())
            .as_secs_f64();
        let animation_updates = update_scheduled_and_discovered_animations(
            tree,
            scheduled_animation_ids,
            frame_time,
            dt,
            discover_animation_work,
        );
        if animation_clock_should_advance(!scheduled_animation_ids.is_empty(), &animation_updates) {
            self.frame_scheduler.animation_advanced(frame_time);
        }
        sync_animation_registrations(active_work, tree, &animation_updates);
        // Declarative animation sources publish their sampled value through
        // State. Consume that reconcile request in the same frame so the
        // sampled value is rendered without scheduling an immediate zero-dt
        // frame ahead of the already outstanding frame opportunity.
        if tree.take_reconcile_requested() {
            *reconcile_pending = true;
        }
        active_work.sync_timers(tree.active_timers(), frame_time);
        self.sync_app_timers(active_work, app_timers);
        let mut animation_frame_token = None;
        if self.frame_scheduler.is_renderable()
            && !self.frame_scheduler.has_outstanding_request()
            && active_work.animation_ids().next().is_some()
        {
            if let Some(token) = self.frame_scheduler.request_animation_frame(frame_time) {
                animation_frame_token = Some(token);
                match platform_window.request_native_frame(NativeFrameRequest::after_present(token))
                {
                    Ok(true) => {
                        self.frame_scheduler.mark_native_armed(token);
                    }
                    Ok(false) => {}
                    Err(error) => {
                        crate::core::log::warn_fn(format!(
                            "[WindowDriver] native frame request failed; fallback remains armed: {}",
                            error.short_what()
                        ));
                    }
                }
            }
        }

        let mut reconcile_ran = false;
        let mut phase_reconcile_us = 0;
        if *reconcile_pending {
            let reconcile_t0 = Instant::now();
            let root = pending_root
                .take()
                .or_else(|| view_factory.and_then(ViewFactorySlot::build));
            if let Some(root) = root {
                ViewAdapter::reconcile_nodes(tree, root);
                reconcile_ran = true;
            }
            *reconcile_pending = false;
            phase_reconcile_us = reconcile_t0.elapsed().as_micros();
        }
        if reconcile_ran {
            sync_animation_registrations(active_work, tree, &[]);
        }

        let native_width = platform_window.properties().width();
        let native_height = platform_window.properties().height();
        if native_width <= 0 || native_height <= 0 {
            self.cancel_outstanding_native_frame(platform_window);
            self.frame_scheduler
                .suspend(SurfaceSuspendReason::ZeroExtent);
            active_work.park_animated_deadlines();
            tree.mark_full_frame_dirty();
            agent_commands.fail_not_presentable();
            *loop_state = next_loop_state(
                tree,
                active_work,
                next_external_deadline,
                false,
                false,
                agent_commands.has_work(),
            );
            self.publish_agent_window_availability(semantic_state, platform_window);
            return WindowFrameResult { did_work: true };
        }
        let surface_corrected =
            ensure_surface_matches_window(tree, engine, native_width, native_height);
        let has_layout = has_layout_work(tree);
        let needs_layout =
            had_layout_event || surface_corrected || !self.rendered_first || has_layout;

        let mut layout_calls_this_frame = 0u32;
        let mut laid_out = false;
        let mut phase_layout_us = 0;
        if needs_layout {
            let layout_t0 = Instant::now();
            let before_version = tree.tree_version();
            tree.layout();
            record_layout(metrics);
            laid_out = true;
            layout_calls_this_frame += 1;

            sync_root_frame_to_engine(tree, engine);
            if let Some(platform) = platform.as_deref_mut() {
                on_frame(tree, engine, platform);
            }
            sync_root_frame_to_engine(tree, engine);

            if tree.tree_version() != before_version {
                tree.layout();
                record_layout(metrics);
                layout_calls_this_frame += 1;
                tree.mark_full_frame_dirty();
            }
            phase_layout_us = layout_t0.elapsed().as_micros();
        }

        let need_render = !self.rendered_first || tree.has_render_work();
        if !self.rendered_first && need_render {
            tree.mark_full_frame_dirty();
            if !laid_out {
                let layout_t0 = Instant::now();
                tree.layout();
                record_layout(metrics);
                layout_calls_this_frame += 1;
                phase_layout_us += layout_t0.elapsed().as_micros();
            }
        }

        semantic_state.refresh(tree);
        observe_agent_settle(
            agent_commands,
            semantic_state,
            tree,
            main_thread_queue,
            pending_root,
            *reconcile_pending,
            false,
            false,
        );

        let dirty_region = tree.dirty_region();
        let dirty_full = dirty_region.full_frame;
        let paint_t0 = Instant::now();
        let (outcome, outcome_source) = if !need_render {
            (RenderOutcome::Idle, InvalidationSource::None)
        } else {
            let theme_ref = theme.borrow();
            let snapshot = ThemeSnapshot::new(theme_ref.tokens());
            let scroll_move = tree.scroll_region_moves();
            let hover_pos = debug_mode.get().then(|| cursor_pos.get());
            let metrics_ref = metrics.map(Cell::get);
            let frame_out = self.frame_renderer.render_frame(
                engine,
                tree,
                FrameRenderInput {
                    rendered_first: self.rendered_first,
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
            (frame_out.outcome, frame_out.inv_source)
        };
        let mut phase_paint_us = paint_t0.elapsed().as_micros();

        if let Some(platform) = platform {
            let window_id = platform_window.window_id();
            let native_window = platform_window.native_handle().native_window();
            sync_window_text_input(
                tree,
                active_work,
                text_input,
                window_id,
                native_window,
                platform,
            );
        }

        let mut phase_present_us = 0u128;
        let mut frame_committed = false;
        let mut frame_failure = None;
        // begin_frame 可完成 GPU 到 Software 的恢复切换，提交分支须使用切换后的能力。
        let engine_capabilities = engine.capabilities();
        match outcome {
            RenderOutcome::Present(_) => {
                if engine_capabilities.uses_external_presenter() {
                    let message =
                        "external presenter path reported final Present before platform submission";
                    crate::core::log::error_fn(format!("[WindowDriver] {message}"));
                    frame_failure = Some(protocol_failure(message));
                    self.rendered_first = false;
                } else {
                    record_present(metrics, outcome_source);
                    self.rendered_first = true;
                    frame_committed = true;
                }
            }
            RenderOutcome::PresentPending(damage) => {
                if !engine_capabilities.uses_external_presenter() {
                    let message = "engine-managed path returned external presentation pending";
                    crate::core::log::error_fn(format!("[WindowDriver] {message}"));
                    frame_failure = Some(protocol_failure(message));
                    self.rendered_first = false;
                } else {
                    let present_t0 = Instant::now();
                    let dpr = engine.device_pixel_ratio();
                    let canvas = engine.canvas_2d();
                    let width = canvas.width();
                    let height = canvas.height();
                    let presenter = platform_window.presenter();
                    let coherency = presenter.present_coherency();
                    let present_surface = presenter.present_surface(width, height, dpr);
                    let present_image = presenter.present_image();
                    let damage_plan = self.present_damage_tracker.plan(
                        coherency,
                        present_surface,
                        present_image,
                        &damage,
                    );
                    match presenter.present(
                        canvas.pixels_mut(),
                        width,
                        height,
                        damage_plan.present_damage,
                    ) {
                        Ok(()) => {
                            self.present_damage_tracker.commit(
                                coherency,
                                present_surface,
                                present_image,
                                &damage,
                            );
                            engine.external_present_succeeded();
                            record_present(metrics, outcome_source);
                            self.rendered_first = true;
                            frame_committed = true;
                        }
                        Err(error) => {
                            engine.external_present_failed(error.clone());
                            crate::core::log::error_fn(format!(
                                "[WindowDriver] external present failed: {}",
                                error.what()
                            ));
                            frame_failure = Some(GraphicsFailure::from_error(error));
                            self.rendered_first = false;
                        }
                    }
                    phase_present_us = present_t0.elapsed().as_micros();
                }
            }
            RenderOutcome::Idle => {
                record_idle(metrics, outcome_source);
                if need_render {
                    let message = "frame renderer returned Idle while render work was pending";
                    crate::core::log::error_fn(format!("[WindowDriver] {message}"));
                    frame_failure = Some(protocol_failure(message));
                    self.rendered_first = false;
                }
            }
            RenderOutcome::FrameReady(_) => {
                let message = "frame renderer returned FrameReady without final presentation";
                crate::core::log::error_fn(format!("[WindowDriver] {message}"));
                frame_failure = Some(protocol_failure(message));
                self.rendered_first = false;
            }
            RenderOutcome::Failed(error) => {
                report_graphics_frame_failure(&error);
                self.rendered_first = false;
                frame_failure = Some(error);
            }
        }

        if frame_committed {
            semantic_state.mark_presented();
            engine.note_presented_at(frame_time);
            sync_graphics_maintenance(active_work, engine);
            self.frame_scheduler
                .presented(frame_time, target_present_time.is_some());
            if let Some(token) = animation_frame_token
                .filter(|token| self.frame_scheduler.outstanding_native_token() == Some(*token))
            {
                if let Err(error) = platform_window.native_frame_presented(token) {
                    crate::core::log::warn_fn(format!(
                        "[WindowDriver] native frame present notification failed; fallback remains armed: {}",
                        error.short_what()
                    ));
                }
            }
        } else if let Some(failure) = frame_failure.as_ref() {
            self.cancel_outstanding_native_frame(platform_window);
            if engine.has_terminal_failure() {
                self.frame_scheduler.mark_terminal_failure();
            } else {
                self.frame_scheduler.frame_failed(failure, frame_time);
            }
        }

        let present_probe = crate::core::perf_probe::take_present();
        let paint_probe = crate::core::perf_probe::take_paint();
        if present_probe.present_us > 0 || present_probe.skipped == 1 {
            phase_present_us = present_probe.present_us;
            phase_paint_us = phase_paint_us.saturating_sub(present_probe.present_us);
        }

        let log_frame = frame_committed
            && (had_events
                || reconcile_ran
                || layout_calls_this_frame > 0
                || crate::core::perf_probe::perf_probe_enabled());
        if log_frame {
            log_frame_metrics(
                frame_t0.elapsed().as_micros(),
                input_us,
                phase_reconcile_us,
                phase_layout_us,
                phase_paint_us,
                phase_present_us,
                had_events,
                reconcile_ran,
                layout_calls_this_frame,
                dirty_full,
                paint_probe,
                present_probe,
            );
        }

        if frame_committed && self.deferred_show {
            if let Err(error) = platform_window.show() {
                crate::core::log::error_fn(format!(
                    "[WindowDriver] deferred show after first present failed: {}",
                    error.short_what()
                ));
            } else if let Err(error) = platform_window.raise() {
                crate::core::log::warn_fn(format!(
                    "[WindowDriver] deferred raise after first present failed: {}",
                    error.short_what()
                ));
            }
            let elapsed = self
                .started_at
                .and_then(|started| frame_time.checked_duration_since(started))
                .unwrap_or_default();
            crate::core::log::info_fn(format!(
                "first_present_ms={} (window revealed after present; no pre-present white flash)",
                elapsed.as_millis()
            ));
            self.deferred_show = false;
        }

        self.publish_agent_window_availability(semantic_state, platform_window);

        #[cfg(feature = "test-harness")]
        if let Some(snapshot) = semantic_state.snapshot() {
            tree.publish_automation_snapshot(
                snapshot.generation,
                snapshot.revision,
                snapshot.presented_revision,
                &snapshot.nodes,
            );
        }

        if frame_committed && (needs_layout || has_layout || need_render) {
            tree.reset_invalidation();
        }

        self.arm_visual_request(frame_time, tree, pending_root, *reconcile_pending);

        if !self.frame_scheduler.is_renderable() {
            active_work.park_animated_deadlines();
        }

        let registered_deadline =
            earliest_deadline(self.frame_scheduler.next_deadline(), next_external_deadline);
        *loop_state = next_loop_state(
            tree,
            active_work,
            registered_deadline,
            self.frame_scheduler.is_renderable(),
            self.frame_scheduler.has_due_opportunity(frame_time),
            agent_commands.has_work(),
        );
        WindowFrameResult { did_work: true }
    }
}

pub(crate) fn sync_graphics_maintenance(
    active_work: &mut ActiveWorkRegistry,
    engine: &dyn GraphicsEngine,
) {
    if let Some(deadline) = engine.idle_resource_deadline() {
        active_work.register(ActiveWorkKind::GraphicsMaintenance, deadline);
    } else {
        active_work.unregister(ActiveWorkKind::GraphicsMaintenance);
    }
}

pub(crate) fn sync_animation_registrations(
    active_work: &mut ActiveWorkRegistry,
    tree: &WidgetTree,
    animation_updates: &[(NodeId, bool)],
) {
    let mut managed_registrations = tree.animated_source_registrations();
    tree.extend_view_transition_registrations(&mut managed_registrations);
    active_work.sync_animated_sources(managed_registrations);
    active_work.sync_component_animations(tree.component_animation_ids());

    for &(id, animating) in animation_updates {
        if active_work.manages_animation(id) {
            continue;
        }
        let kind = ActiveWorkKind::Animation(id);
        if animating {
            active_work.register_open(kind);
        } else {
            active_work.unregister(kind);
        }
    }
}
