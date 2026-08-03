use super::*;

impl WindowDriver {
    pub(crate) fn new(width: i32, height: i32, deferred_show: bool) -> Self {
        Self {
            frame_renderer: ScenePipeline::new(),
            rendered_first: false,
            present_damage_tracker: PresentDamageTracker::new(),
            frame_scheduler: FrameScheduler::new(width > 0 && height > 0),
            last_frame: None,
            initial_size: (width, height),
            deferred_show,
            started_at: None,
            presented_sequence: 0,
            scheduled_animation_ids_scratch: Vec::new(),
            app_timer_deadlines_scratch: Vec::new(),
            app_timer_deadline_revision: None,
            due_work_scratch: Vec::new(),
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
        let Some(revision) = app_timers.deadlines_into_if_changed(
            self.app_timer_deadline_revision,
            &mut self.app_timer_deadlines_scratch,
        ) else {
            return;
        };
        active_work.sync_app_timers(self.app_timer_deadlines_scratch.iter().copied());
        self.app_timer_deadline_revision = Some(revision);
    }

    /// Applies the shared native window lifecycle portion of an event.
    ///
    /// Returns whether the event is a layout-affecting window event. Semantic
    /// dispatch remains in the outer loop so its event mapper stays injectable.
    pub(crate) fn handle_window_event(
        &mut self,
        event: &UiEvent,
        tree: &mut WidgetTree,
        engine: &mut dyn RenderTarget,
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
                if engine.logical_extent() == self.initial_size {
                    let width;
                    let height;
                    match platform.display().info(0) {
                        Ok(info) => {
                            width = info.bounds.w as i32;
                            height = info.bounds.h as i32;
                        }
                        Err(error) => {
                            tracing::warn!(
                                "window maximize: display info unavailable: {}",
                                error.short_what()
                            );
                            width = 0;
                            height = 0;
                        }
                    }
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
    pub(super) fn suspend_if_surface_unavailable(
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

    pub(super) fn arm_visual_request(
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
}
