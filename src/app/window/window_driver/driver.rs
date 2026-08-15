use super::*;

impl WindowDriver {
    pub(crate) fn new(width: i32, height: i32, deferred_show: bool) -> Self {
        Self {
            frame_renderer: ScenePipeline::new(),
            rendered_first: false,
            present_damage_tracker: PresentDamageTracker::new(),
            frame_scheduler: FrameScheduler::new(width > 0 && height > 0),
            last_frame: None,
            deferred_show,
            started_at: None,
            presented_sequence: 0,
            scheduled_animation_ids_scratch: Vec::new(),
            app_timer_deadlines_scratch: Vec::new(),
            app_timer_deadline_revision: None,
            due_work_scratch: Vec::new(),
            // 诊断统计从零开始，计时起点由 Default 取当前时刻。
            frame_diag: FrameDiagnostics::default(),
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

// 窗口状态事实与几何事务分离的回归测试。
#[cfg(all(test, feature = "test-harness"))]
mod tests {
    // 复用窗口驱动内部契约和测试所需类型。
    use super::*;
    // 使用只记录命令的 RenderTarget 观测 logical extent。
    use crate::draw::painting::recorder::CommandRecorder;
    // 使用内存窗口观测 resize_notify 调用。
    use crate::native::test_harness::FakeWindow;

    // 构造不携带几何的窗口状态事实。
    fn lifecycle_event(type_: UiEventType) -> UiEvent {
        // 返回只表达最大化或还原状态的原生事件。
        UiEvent {
            // 单窗口驱动已确定路由目标，事件无需重复携带身份。
            window_id: None,
            // 保存调用方要求的窗口状态事实类型。
            type_,
            // 状态事实不携带客户区尺寸。
            payload: UiEventPayload::None,
        }
    }

    // 验证最大化与还原只消费状态事实，唯一 resize 来自 WindowResize。
    #[test]
    fn maximize_restore_keeps_resize_as_single_geometry_authority() {
        // 以 800x600 初始逻辑客户区创建窗口驱动。
        let mut driver = WindowDriver::new(800, 600, false);
        // 创建可接收 full-frame dirty 的空组件树。
        let mut tree = WidgetTree::new();
        // 创建记录 logical extent 的测试渲染目标。
        let mut engine = CommandRecorder::new();
        // 初始化渲染目标到窗口初始尺寸。
        engine.initialize(800, 600).expect("初始化测试渲染目标");
        // 创建记录平台 resize_notify 的测试窗口。
        let mut window = FakeWindow::new(1, "测试窗口", 800, 600);
        // 创建窗口独占的文本输入状态。
        let mut text_input = WindowTextInputState::default();

        // 先投递两次最大化状态事实，模拟重复原生通知。
        for _ in 0..2 {
            // 最大化事实只改变调度和 dirty，不得触发几何事务。
            assert!(driver.handle_window_event(
                &lifecycle_event(UiEventType::WindowMaximize),
                &mut tree,
                &mut engine,
                &mut window,
                &mut text_input,
            ));
        }
        // 最大化事实不能用显示器 bounds 改写渲染目标。
        assert_eq!(engine.logical_extent(), (800, 600));
        // 最大化事实不能重复调用平台 resize_notify。
        assert!(window.state.resize_notify_calls.is_empty());
        // 投递最大化后的权威逻辑客户区 resize。
        let maximized_resize = UiEvent::resize(1920, 1040);
        // WindowResize 独占一次 graphics 与平台 resize 事务。
        assert!(driver.handle_window_event(
            &maximized_resize,
            &mut tree,
            &mut engine,
            &mut window,
            &mut text_input,
        ));
        // 渲染目标采用原生事件给出的当前逻辑客户区尺寸。
        assert_eq!(engine.logical_extent(), (1920, 1040));
        // 平台窗口只接收一次相同尺寸的 resize_notify。
        assert_eq!(window.state.resize_notify_calls, vec![(1920, 1040)]);

        // 再投递两次还原状态事实，覆盖重复最大化切换序列。
        for _ in 0..2 {
            // 还原事实只改变调度和 dirty，不得回放初始尺寸。
            assert!(driver.handle_window_event(
                &lifecycle_event(UiEventType::WindowRestore),
                &mut tree,
                &mut engine,
                &mut window,
                &mut text_input,
            ));
        }
        // 还原事实到权威 resize 到达前必须保留当前 surface extent。
        assert_eq!(engine.logical_extent(), (1920, 1040));
        // 还原事实不得增加平台 resize_notify 次数。
        assert_eq!(window.state.resize_notify_calls, vec![(1920, 1040)]);

        // 投递还原后的权威逻辑客户区 resize。
        let restored_resize = UiEvent::resize(800, 600);
        // WindowResize 再独占一次 geometry/surface 事务。
        assert!(driver.handle_window_event(
            &restored_resize,
            &mut tree,
            &mut engine,
            &mut window,
            &mut text_input,
        ));
        // 渲染目标最终回到原生事件报告的还原尺寸。
        assert_eq!(engine.logical_extent(), (800, 600));
        // 两次权威 resize 各产生且只产生一次平台通知。
        assert_eq!(
            window.state.resize_notify_calls,
            vec![(1920, 1040), (800, 600)]
        );

        // 再覆盖 Wayland 的 resize 先于状态事实顺序。
        let resize_before_maximize = UiEvent::resize(1600, 900);
        // 先到的权威 resize 正常推进第三次几何事务。
        assert!(driver.handle_window_event(
            &resize_before_maximize,
            &mut tree,
            &mut engine,
            &mut window,
            &mut text_input,
        ));
        // 后到的最大化事实不得再次改写已采用的客户区尺寸。
        assert!(driver.handle_window_event(
            &lifecycle_event(UiEventType::WindowMaximize),
            &mut tree,
            &mut engine,
            &mut window,
            &mut text_input,
        ));
        // resize 后状态顺序仍保留权威 logical extent。
        assert_eq!(engine.logical_extent(), (1600, 900));
        // 最大化事实没有增加第四次平台 resize_notify。
        assert_eq!(
            window.state.resize_notify_calls,
            vec![(1920, 1040), (800, 600), (1600, 900)]
        );

        // 覆盖 resize 先于还原事实的反向切换。
        let resize_before_restore = UiEvent::resize(800, 600);
        // 先到的还原尺寸正常推进第四次几何事务。
        assert!(driver.handle_window_event(
            &resize_before_restore,
            &mut tree,
            &mut engine,
            &mut window,
            &mut text_input,
        ));
        // 后到的还原事实不得回放其他历史尺寸。
        assert!(driver.handle_window_event(
            &lifecycle_event(UiEventType::WindowRestore),
            &mut tree,
            &mut engine,
            &mut window,
            &mut text_input,
        ));
        // 最终 logical extent 与最后一个权威 resize 完全一致。
        assert_eq!(engine.logical_extent(), (800, 600));
        // 四次权威 resize 与四次平台通知保持一一对应。
        assert_eq!(
            window.state.resize_notify_calls,
            vec![(1920, 1040), (800, 600), (1600, 900), (800, 600)]
        );
    }
}
