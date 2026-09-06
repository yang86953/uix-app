use super::*;

impl WindowDriver {
    pub(crate) fn new(
        width: i32,
        height: i32,
        deferred_show: bool,
        diagnostics: Diagnostics,
    ) -> Self {
        Self {
            frame_renderer: ScenePipeline::new(),
            rendered_first: false,
            present_damage_tracker: PresentDamageTracker::new(),
            frame_scheduler: FrameScheduler::new(width > 0 && height > 0),
            last_frame: None,
            deferred_show,
            // 窗口在收到首个 WindowFocus 事件前按未聚焦处理。
            window_focused: false,
            started_at: None,
            presented_sequence: 0,
            scheduled_animation_ids_scratch: Vec::new(),
            animation_updates_scratch: Vec::new(),
            animation_registrations_scratch: Vec::new(),
            app_timer_deadlines_scratch: Vec::new(),
            app_timer_deadline_revision: None,
            due_work_scratch: Vec::new(),
            // 诊断统计从零开始，计时起点由 Default 取当前时刻。
            frame_diag: FrameDiagnostics::default(),
            terminal_failure_reported: false,
            diagnostics,
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
        text_input: &mut WindowTextInputState,
        diagnostics: &Diagnostics,
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
                            diagnostics,
                            "window graphics resize failed",
                            engine.resize(data.width, data.height),
                        );
                        report_window_operation_error(
                            diagnostics,
                            "window resize_notify failed",
                            platform_window.resize_notify(data.width, data.height),
                        );
                        if resized {
                            // Surface resize 会丢弃旧代际内容；下一帧必须从空白目标完整重建，
                            // 不能让还原后的较小窗口复用最大化帧的 retained 像素。
                            self.rendered_first = false;
                            self.present_damage_tracker.reset();
                            sync_root_frame_exactly_to_engine(tree, engine);
                        }
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
                // 状态事实只恢复调度；紧随或先到的 WindowResize 唯一拥有几何事务。
                self.cancel_outstanding_native_frame(platform_window);
                // 最大化可能从最小化状态进入，恢复可呈现状态但不合成显示器尺寸。
                self.frame_scheduler.resume();
                // 状态切换仍要求下一帧完整重绘，避免复用切换前的布局与 retained 内容。
                tree.mark_full_frame_dirty();
                true
            }
            UiEventType::WindowRestore => {
                // 还原事实不回放初始配置尺寸；原生 WindowResize 提供当前客户区真相。
                self.cancel_outstanding_native_frame(platform_window);
                // 从最小化还原时恢复调度；普通最大化还原保持当前可呈现代际。
                self.frame_scheduler.resume();
                // 状态切换仍要求下一帧完整重绘，实际 surface generation 由 resize 推进。
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
                // 焦点事实同步到逐窗驱动状态，供 Agent 目录发布可观测的
                // focused 字段；未聚焦窗口的指针与键盘输入会被树层门禁忽略。
                self.window_focused = true;
                false
            }
            UiEventType::WindowBlur => {
                text_input.window_focused = false;
                self.window_focused = false;
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
        if !platform_window.is_offscreen() && !platform_window.is_visible() {
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
                        // 原生 surface 可用性探测 deadline。
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

        self.arm_visual_request(now, tree, pending_root, *reconcile_pending, false, false);

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
        // 剩余主线程任务必须形成即时 deadline，保证预算耗尽后继续调度。
        main_thread_queue: &MainThreadQueue,
        agent_commands: &WindowAgentState,
        pending_root: &Option<crate::ui::view::ViewNode>,
        reconcile_pending: bool,
    ) -> Option<Instant> {
        self.sync_app_timers(active_work, app_timers);
        if tree.take_reconcile_requested() {
            self.arm_visual_request(now, tree, pending_root, true, false, false);
        } else {
            self.arm_visual_request(now, tree, pending_root, reconcile_pending, false, false);
        }
        earliest_deadline(
            // 队列剩余任务与 Agent 工作都要求下一窗口轮次立即运行。
            (!main_thread_queue.is_empty() || agent_commands.has_work()).then_some(now),
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
        animation_continue: bool,
        allow_tighten: bool,
    ) {
        if !self.frame_scheduler.is_renderable() {
            return;
        }
        if !self.rendered_first
            || pending_root.is_some()
            || reconcile_pending
            // 作用域重建请求同样需要一个渲染机会才能被协调段消费。
            || tree.has_scoped_rebuild_requested()
            || has_invalidation_work(tree)
        {
            // 动画延续帧按 fallback cadence 武装（request_frame 只接受更早
            // deadline，因此不会收紧已武装的 cadence 请求）。
            if animation_continue {
                self.frame_scheduler.request_animation_frame(now);
            } else if allow_tighten || !self.frame_scheduler.has_outstanding_request() {
                // 收紧现有请求只允许发生在帧入口的外部唤醒路径；空闲探测与
                // 帧尾续帧在已有请求（尤其 cadence 请求）时不得改为立即，
                // 否则动画 tick 的残留失效会把每个动画帧收紧成全速帧链。
                self.frame_scheduler.request_immediate(now);
            }
        }
    }
}
