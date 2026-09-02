use super::*;

impl WindowDriver {
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
            debug_correlation_id,
            hud_state,
            cursor_pos,
            metrics,
            now,
            had_events,
            had_layout_event,
            next_external_deadline,
            on_runtime_tasks,
            on_frame,
        } = context;
        // 协调中的树暂时拒绝重入帧，但不得把可成功提交的事务误判为永久失败。
        if !tree.accepts_external_work() && !tree.is_fail_stopped() {
            // 保留既有调度事实，由最外层协调结束后的正常帧继续消费。
            return WindowFrameResult { did_work: false };
        }
        // 永久停止的树只允许窗口所有者收敛调度资源，不得继续执行帧管线。
        if tree.is_fail_stopped() {
            // 统一释放逐窗调度资源并停止当前帧。
            return self.finish_fail_stopped_frame(
                // 清空当前窗口的活动工作。
                active_work,
                // 取消当前窗口的应用定时器。
                app_timers,
                // 丢弃当前窗口的主线程队列。
                main_thread_queue,
                // 关闭当前窗口的 Agent 状态。
                agent_commands,
                // 释放当前窗口的待协调根。
                pending_root,
                // 清除当前窗口的协调请求位。
                reconcile_pending,
                // 收敛当前窗口循环状态。
                loop_state,
            );
        }
        self.started_at.get_or_insert(now);
        self.publish_agent_window_availability(semantic_state, platform_window, engine);

        // 关闭调试且未显式配置卡顿阈值时，不读取时钟或扫描现场。
        let collect_frame_diagnostics = frame_diagnostics_enabled(debug_mode.debug_mode());
        // 帧诊断：仅在启用时记录本帧起点。
        let frame_start = collect_frame_diagnostics.then(Instant::now);
        // 帧诊断：各阶段耗时累计变量。
        let mut layout_us = Duration::ZERO;
        let mut render_us = Duration::ZERO;
        let mut present_us = Duration::ZERO;
        // 帧诊断：GPU 提交阶段耗时（仅 GPU 路径填充，其余保持零）。
        let mut submit_us = Duration::ZERO;

        active_work.sync_timers(tree.active_timers(), now);
        self.sync_app_timers(active_work, app_timers);
        active_work.drain_due_into(now, &mut self.due_work_scratch);
        let due_work = self.due_work_scratch.as_slice();
        let had_registered_work = !due_work.is_empty();
        if due_work.contains(&ActiveWorkKind::GraphicsMaintenance) {
            engine.release_idle_resources(now);
            sync_graphics_maintenance(active_work, engine);
        }
        let had_due_animation_work = due_work
            .iter()
            .any(|work| matches!(work, ActiveWorkKind::Animation(_)));
        let had_due_widget_timer_work = with_platform_clipboard(&mut platform, || {
            dispatch_due_active_work(tree, app_timers, due_work, now)
        });
        // 到期计时器可能在内部捕获发布 panic，必须在消费下一类队列前停止本帧。
        if let Some(result) = self.finish_if_tree_fail_stopped(
            // 检查本窗口唯一拥有的树状态。
            tree,
            // 释放当前窗口的活动工作。
            active_work,
            // 取消当前窗口的应用计时器。
            app_timers,
            // 清空当前窗口的主线程队列。
            main_thread_queue,
            // 关闭当前窗口的 Agent 命令端口。
            agent_commands,
            // 释放尚未协调的声明根。
            pending_root,
            // 清除协调请求位。
            reconcile_pending,
            // 令窗口循环进入深度空闲。
            loop_state,
        ) {
            // fail-stop 不得继续处理本帧工作。
            return result;
        }

        let had_main_thread_work = {
            // 主线程任务只在当前逐窗 owner turn 内短借原生窗口。
            let mut main_thread_context =
                MainThreadContext::new(pending_root, reconcile_pending, platform_window);
            main_thread_queue.drain(&mut main_thread_context)
        };
        // 主线程任务可能在内部捕获发布 panic，必须在处理 Agent 工作前停止本帧。
        if let Some(result) = self.finish_if_tree_fail_stopped(
            // 检查本窗口唯一拥有的树状态。
            tree,
            // 释放当前窗口的活动工作。
            active_work,
            // 取消当前窗口的应用计时器。
            app_timers,
            // 清空当前窗口的主线程队列。
            main_thread_queue,
            // 关闭当前窗口的 Agent 命令端口。
            agent_commands,
            // 释放尚未协调的声明根。
            pending_root,
            // 清除协调请求位。
            reconcile_pending,
            // 令窗口循环进入深度空闲。
            loop_state,
        ) {
            // fail-stop 不得继续处理本帧工作。
            return result;
        }
        let had_agent_pending = agent_commands.has_work();
        let presentable = self.agent_surface_presentable(platform_window);
        let mut window_ops = PlatformWindowAgentOps::new(platform_window);
        let had_agent_command_work =
            agent_commands.drain_ready(tree, semantic_state, presentable, &mut window_ops);
        // Agent 命令可能在内部捕获发布 panic，必须在处理 AppState 前停止本帧。
        if let Some(result) = self.finish_if_tree_fail_stopped(
            // 检查本窗口唯一拥有的树状态。
            tree,
            // 释放当前窗口的活动工作。
            active_work,
            // 取消当前窗口的应用计时器。
            app_timers,
            // 清空当前窗口的主线程队列。
            main_thread_queue,
            // 关闭当前窗口的 Agent 命令端口。
            agent_commands,
            // 释放尚未协调的声明根。
            pending_root,
            // 清除协调请求位。
            reconcile_pending,
            // 令窗口循环进入深度空闲。
            loop_state,
        ) {
            // fail-stop 不得继续处理本帧工作。
            return result;
        }
        let had_app_state_focus_work =
            with_platform_clipboard(&mut platform, || tree.drain_app_state_focus_requests());
        let had_app_state_semantic_work =
            with_platform_clipboard(&mut platform, || tree.drain_app_state_semantic_events());
        // AppState 事件可能在内部捕获发布 panic，必须在同步调度前停止本帧。
        if let Some(result) = self.finish_if_tree_fail_stopped(
            // 检查本窗口唯一拥有的树状态。
            tree,
            // 释放当前窗口的活动工作。
            active_work,
            // 取消当前窗口的应用计时器。
            app_timers,
            // 清空当前窗口的主线程队列。
            main_thread_queue,
            // 关闭当前窗口的 Agent 命令端口。
            agent_commands,
            // 释放尚未协调的声明根。
            pending_root,
            // 清除协调请求位。
            reconcile_pending,
            // 令窗口循环进入深度空闲。
            loop_state,
        ) {
            // fail-stop 不得继续处理本帧工作。
            return result;
        }
        active_work.sync_timers(tree.active_timers(), now);
        self.sync_app_timers(active_work, app_timers);
        if let Some(platform) = platform.as_deref_mut() {
            on_runtime_tasks(platform, tree);
        }
        // Runtime 任务可能在内部捕获发布 panic，必须在消费协调请求前停止本帧。
        if let Some(result) = self.finish_if_tree_fail_stopped(
            // 检查本窗口唯一拥有的树状态。
            tree,
            // 释放当前窗口的活动工作。
            active_work,
            // 取消当前窗口的应用计时器。
            app_timers,
            // 清空当前窗口的主线程队列。
            main_thread_queue,
            // 关闭当前窗口的 Agent 命令端口。
            agent_commands,
            // 释放尚未协调的声明根。
            pending_root,
            // 清除协调请求位。
            reconcile_pending,
            // 令窗口循环进入深度空闲。
            loop_state,
        ) {
            // fail-stop 不得继续处理本帧工作。
            return result;
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
        // Effect 若在 tick 内部捕获发布 panic，必须在读取协调请求前停止本帧。
        if let Some(result) = self.finish_if_tree_fail_stopped(
            // 检查本窗口唯一拥有的树状态。
            tree,
            // 释放当前窗口的活动工作。
            active_work,
            // 取消当前窗口的应用计时器。
            app_timers,
            // 清空当前窗口的主线程队列。
            main_thread_queue,
            // 关闭当前窗口的 Agent 命令端口。
            agent_commands,
            // 释放尚未协调的声明根。
            pending_root,
            // 清除协调请求位。
            reconcile_pending,
            // 令窗口循环进入深度空闲。
            loop_state,
        ) {
            // fail-stop 不得继续处理本帧工作。
            return result;
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

        // Agent 动作绑定的是进程内窗口，不借用合成器前台状态。surface 不可呈现时
        // 仍完成声明协调、布局和语义刷新；只把 paint/present 与截屏留给可呈现路径。
        if !presentable && agent_commands.has_background_in_flight() {
            let mut reconcile_ran = false;
            if *reconcile_pending {
                let root = pending_root.take().or_else(|| {
                    view_factory.and_then(|factory| factory.build(tree.widget_state_store()))
                });
                if let Some(root) = root {
                    ViewAdapter::reconcile_nodes(tree, root);
                    reconcile_ran = true;
                }
                *reconcile_pending = false;
            }
            // 作用域重建：根协调已重跑全部作用域闭包时请求作废；否则逐节点原位重建。
            let mut scoped_ran = false;
            if tree.has_scoped_rebuild_requested() {
                let requests = if reconcile_ran {
                    tree.drop_scoped_rebuild_requests();
                    Vec::new()
                } else {
                    tree.take_scoped_rebuild_requests()
                };
                for id in requests {
                    ViewAdapter::reconcile_scoped_node(tree, id);
                    scoped_ran = true;
                }
            }
            if reconcile_ran || scoped_ran {
                sync_animation_registrations(
                    active_work,
                    tree,
                    &[],
                    &mut self.animation_registrations_scratch,
                );
            }
            if has_layout_work(tree) {
                tree.layout();
                record_layout(metrics);
            }
            if let Some(result) = self.finish_if_tree_fail_stopped(
                tree,
                active_work,
                app_timers,
                main_thread_queue,
                agent_commands,
                pending_root,
                reconcile_pending,
                loop_state,
            ) {
                return result;
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
        }

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
            self.publish_agent_window_availability(semantic_state, platform_window, engine);
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
                    self.publish_agent_window_availability(semantic_state, platform_window, engine);
                    return WindowFrameResult { did_work: true };
                }
                Err(error) => {
                    tracing::error!(
                        "[WindowDriver] occlusion present test failed: {}",
                        error.short_what()
                    );
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
                    self.publish_agent_window_availability(semantic_state, platform_window, engine);
                    return WindowFrameResult { did_work: true };
                }
            }
        }

        if had_due_animation_work {
            self.frame_scheduler.request_immediate(now);
        }
        // 帧入口的续帧服务于本次外部唤醒（输入、定时器等）：允许收紧已武装
        // 的 cadence 请求，让输入在同一轮渲染，保证响应延迟不受动画节奏约束。
        self.arm_visual_request(now, tree, pending_root, *reconcile_pending, false, true);
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
            self.publish_agent_window_availability(semantic_state, platform_window, engine);
            return WindowFrameResult {
                did_work: event_work,
            };
        };

        if let Some(token) = opportunity.fallback_token() {
            if let Err(error) = platform_window.cancel_native_frame(token) {
                // 逐帧取消失败是瞬态平台噪声：调度器保持自身状态，经
                // observe_transient 冷却去重观察。
                self.diagnostics.observe_transient_error(
                    "window_driver",
                    "fallback native frame cancellation failed", &error);
            }
        }
        let frame_time = opportunity.frame_time();
        let target_present_time = opportunity.target_present_time();
        self.last_frame = Some(frame_time);
        self.scheduled_animation_ids_scratch.clear();
        self.scheduled_animation_ids_scratch
            .extend(active_work.animation_ids());
        // 先保存是否存在已调度动画，避免让切片借用跨越 fail-stop helper。
        let had_scheduled_animation_work = !self.scheduled_animation_ids_scratch.is_empty();
        let discover_animation_work = event_work
            || !self.rendered_first
            || *reconcile_pending
            // 作用域重建请求同样驱动本轮声明工作发现。
            || tree.has_scoped_rebuild_requested()
            || has_invalidation_work(tree);
        let dt = self
            .frame_scheduler
            .animation_delta(frame_time, had_scheduled_animation_work)
            .as_secs_f64();
        let mut animation_updates = std::mem::take(&mut self.animation_updates_scratch);
        update_scheduled_and_discovered_animations(
            tree,
            self.scheduled_animation_ids_scratch.as_slice(),
            frame_time,
            dt,
            discover_animation_work,
            &mut animation_updates,
        );
        // 动画更新可能在内部捕获发布 panic，必须在推进时钟前停止本帧。
        if let Some(result) = self.finish_if_tree_fail_stopped(
            // 检查本窗口唯一拥有的树状态。
            tree,
            // 释放当前窗口的活动工作。
            active_work,
            // 取消当前窗口的应用计时器。
            app_timers,
            // 清空当前窗口的主线程队列。
            main_thread_queue,
            // 关闭当前窗口的 Agent 命令端口。
            agent_commands,
            // 释放尚未协调的声明根。
            pending_root,
            // 清除协调请求位。
            reconcile_pending,
            // 令窗口循环进入深度空闲。
            loop_state,
        ) {
            // fail-stop 不得继续处理本帧工作。
            self.animation_updates_scratch = animation_updates;
            return result;
        }
        // Helper 返回后使用预存事实判断，保持动画时钟推进语义不变。
        if animation_clock_should_advance(had_scheduled_animation_work, &animation_updates) {
            self.frame_scheduler.animation_advanced(frame_time);
        }
        let had_animation_updates = !animation_updates.is_empty();
        sync_animation_registrations(
            active_work,
            tree,
            &animation_updates,
            &mut self.animation_registrations_scratch,
        );
        self.animation_updates_scratch = animation_updates;
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
            && platform_window
                .capabilities()
                .supports(WindowCapability::RequestNativeFrame)
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
                        // 帧请求失败保留 fallback 武装状态，下个机会重试：
                        // 经 observe_transient 冷却去重观察。
                        self.diagnostics.observe_transient_error(
                            "window_driver",
                            "native frame request failed; fallback remains armed", &error);
                    }
                }
            }
        }

        let mut reconcile_ran = false;
        // 帧诊断：记录本帧协调前的树版本，用于判断是否发生了整树重建。
        let pre_reconcile_version = tree.tree_version();
        if *reconcile_pending {
            let root = pending_root
                .take()
                // 让根工厂在协调时复用该窗口树拥有的组件私有状态。
                .or_else(|| {
                    view_factory.and_then(|factory| factory.build(tree.widget_state_store()))
                });
            if let Some(root) = root {
                ViewAdapter::reconcile_nodes(tree, root);
                reconcile_ran = true;
            }
            *reconcile_pending = false;
        }
        // 作用域重建：根协调已重跑全部作用域闭包时请求作废；否则逐节点原位重建。
        let mut scoped_ran = false;
        if tree.has_scoped_rebuild_requested() {
            let requests = if reconcile_ran {
                // 根协调的输出已覆盖每个作用域子树，残余请求全部作废。
                tree.drop_scoped_rebuild_requests();
                Vec::new()
            } else {
                tree.take_scoped_rebuild_requests()
            };
            for id in requests {
                ViewAdapter::reconcile_scoped_node(tree, id);
                scoped_ran = true;
            }
        }
        if reconcile_ran || scoped_ran {
            sync_animation_registrations(
                active_work,
                tree,
                &[],
                &mut self.animation_registrations_scratch,
            );
        }
        // 没有开放帧动画时立即释放峰值工作区，延迟等待和空闲期不长期保留容量。
        if active_work.animation_ids().next().is_none() {
            self.animation_updates_scratch = Vec::new();
            self.animation_registrations_scratch = Vec::new();
        }
        // 协调内部捕获发布 panic 后不得让同一帧观察半提交结构。
        if let Some(result) = self.finish_if_tree_fail_stopped(
            // 检查本窗口唯一拥有的树状态。
            tree,
            // 释放当前窗口的活动工作。
            active_work,
            // 取消当前窗口的应用计时器。
            app_timers,
            // 清空当前窗口的主线程队列。
            main_thread_queue,
            // 关闭当前窗口的 Agent 命令端口。
            agent_commands,
            // 释放尚未协调的声明根。
            pending_root,
            // 清除协调请求位。
            reconcile_pending,
            // 令窗口循环进入深度空闲。
            loop_state,
        ) {
            // fail-stop 不得继续处理本帧工作。
            return result;
        }

        let (native_width, native_height) = native_client_logical_extent(platform_window);
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
            self.publish_agent_window_availability(semantic_state, platform_window, engine);
            return WindowFrameResult { did_work: true };
        }
        let surface_corrected =
            ensure_surface_matches_window(tree, engine, native_width, native_height, &self.diagnostics);
        let has_layout = has_layout_work(tree);
        let needs_layout =
            had_layout_event || surface_corrected || !self.rendered_first || has_layout;

        let mut laid_out = false;
        if needs_layout {
            // 帧诊断：布局阶段起点。
            let layout_start = collect_frame_diagnostics.then(Instant::now);
            tree.layout();
            record_layout(metrics);
            laid_out = true;

            sync_root_frame_to_engine(tree, engine);
            let before_on_frame_version = tree.tree_version();
            if let Some(platform) = platform.as_deref_mut() {
                on_frame(tree, engine, platform);
            }
            // on_frame 若捕获协调发布 panic，禁止继续同步或渲染半树。
            if let Some(result) = self.finish_if_tree_fail_stopped(
                // 检查本窗口唯一拥有的树状态。
                tree,
                // 释放当前窗口的活动工作。
                active_work,
                // 取消当前窗口的应用计时器。
                app_timers,
                // 清空当前窗口的主线程队列。
                main_thread_queue,
                // 关闭当前窗口的 Agent 命令端口。
                agent_commands,
                // 释放尚未协调的声明根。
                pending_root,
                // 清除协调请求位。
                reconcile_pending,
                // 令窗口循环进入深度空闲。
                loop_state,
            ) {
                // fail-stop 不得继续处理本帧工作。
                return result;
            }
            sync_root_frame_to_engine(tree, engine);

            let after_on_frame_version = tree.tree_version();
            // 第一轮已原子消费原有 Layout；只有布局内部或 on_frame 新产生的
            // Layout 请求才需要同帧再收敛，避免仅因合成拓扑版本变化重复整树布局。
            if has_layout_work(tree) {
                tree.layout();
                record_layout(metrics);
            }
            // on_frame 可能替换根或改变应用结构，必须保守清除旧像素；
            // 首轮布局自身的版本变化已由布局 damage 精确覆盖，无需升为全帧。
            if after_on_frame_version != before_on_frame_version {
                tree.mark_full_frame_dirty();
            }
            // 帧诊断：布局阶段耗时。
            layout_us = layout_start.map_or(Duration::ZERO, |start| start.elapsed());
        }

        let need_render = !self.rendered_first || tree.has_render_work();
        if !self.rendered_first && need_render {
            tree.mark_full_frame_dirty();
            if !laid_out {
                tree.layout();
                record_layout(metrics);
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

        let invalidation_revision_before_render = tree.invalidation_revision();
        let dirty_region = tree.dirty_region();
        // 管线接管脏区 Vec 前先保存诊断标量，避免为低频日志保留第二份矩形快照。
        let dirty_diagnostics = collect_frame_diagnostics.then(|| {
            let rendered_full = dirty_region.full_frame || (debug_mode.debug_mode() && need_render);
            let dirty_area_pct = if rendered_full {
                1.0
            } else {
                let bounds = dirty_region.bounds();
                let area = bounds.w * bounds.h;
                let total = (native_width as f32) * (native_height as f32);
                if total > 0.0 {
                    (area / total) as f64
                } else {
                    0.0
                }
            };
            let dirty_rect_count = if rendered_full {
                0
            } else {
                dirty_region.rects().len()
            };
            (rendered_full, dirty_area_pct, dirty_rect_count)
        });
        // 帧诊断：渲染前读取失效队列（帧尾时队列已被消费，无法反映本帧失效来源）。
        let (inval_count, inval_biggest) = invalidation_diag(tree);
        // 记录 GPU 首帧前已经成功显示的窗口，避免成功后重复调用 show。
        let mut pre_present_shown = false;
        let (outcome, outcome_source) = if !need_render {
            (RenderOutcome::Idle, InvalidationSource::None)
        } else {
            let theme_ref = theme.borrow();
            tree.set_theme_tokens(theme_ref.tokens_arc());
            let scroll_move = tree.scroll_region_moves();
            let hover_pos = debug_mode.debug_mode().then(|| cursor_pos.get());
            let invalidation_source = if !self.rendered_first {
                InvalidationSource::FirstFrame
            } else if had_layout_event || surface_corrected {
                InvalidationSource::LayoutEvent
            } else if had_due_animation_work
                || had_scheduled_animation_work
                || had_animation_updates
            {
                InvalidationSource::AnimationPolling
            } else {
                InvalidationSource::DirtyRegion
            };
            // paint_calls 统计实际进入场景渲染阶段的帧，而不是脏节点猜测值。
            record_paint(metrics);
            let metrics_ref = metrics.map(Cell::get);
            // 原生 swapchain 必须在首个 GPU Present 前进入可见状态，避免隐藏窗口被视为 occluded。
            if self.deferred_show && !engine.capabilities().uses_external_presenter() {
                // 只提前执行可见性切换，保留首帧成功后的统一 show/raise 收尾和失败重试状态。
                match platform_window.show() {
                    // 成功显示后仍保留 deferred_show，直到首帧真正提交才消费状态。
                    Ok(()) => pre_present_shown = true,
                    // 显示失败不丢弃 deferred_show，后续首帧提交仍可重试原有边界。
                    Err(error) => {
                        self.diagnostics.observe_transient_error(
                            "window_driver",
                            "pre-present show failed; deferred retry remains armed", &error);
                    }
                }
            }
            // 帧诊断：渲染阶段起点。
            let render_start = collect_frame_diagnostics.then(Instant::now);
            let frame_out = self.frame_renderer.render_frame(
                engine,
                tree,
                FrameRenderInput {
                    rendered_first: self.rendered_first,
                    dirty_region,
                    tree_version: tree.tree_version(),
                    scroll_move,
                    font: font_service.loaded_font_handle,
                    font_service,
                    image_service,
                    debug_mode: debug_mode.debug_mode(),
                    hover_pos,
                    metrics: metrics_ref.as_ref(),
                    debug_frame: self.frame_diag.last_frame.as_ref(),
                    hud: hud_state,
                    invalidation_source,
                },
            );
            // 帧诊断：渲染阶段耗时。
            render_us = render_start.map_or(Duration::ZERO, |start| start.elapsed());
            // 帧诊断：读取 GPU 路径的阶段耗时细分（记录/提交）。
            if collect_frame_diagnostics {
                let (_record_us, measured_submit_us) = self.frame_renderer.last_frame_stage_times();
                // 保存 GPU 提交耗时，避免局部绑定遮蔽帧级诊断槽。
                submit_us = measured_submit_us;
            }
            (frame_out.outcome, frame_out.inv_source)
        };
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
                debug_mode,
            );
        }

        let mut frame_committed = false;
        let mut frame_failure = None;
        // begin_frame 可完成 GPU 到 Software 的恢复切换，提交分支须使用切换后的能力。
        let engine_capabilities = engine.capabilities();
        match outcome {
            RenderOutcome::Present(_) => {
                if engine_capabilities.uses_external_presenter() {
                    let message =
                        "external presenter path reported final Present before platform submission";
                    tracing::error!("[WindowDriver] {message}");
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
                    let message = "backend-managed path returned external presentation pending";
                    tracing::error!("[WindowDriver] {message}");
                    frame_failure = Some(protocol_failure(message));
                    self.rendered_first = false;
                } else {
                    let dpr = engine.device_pixel_ratio();
                    let canvas = engine.canvas_2d();
                    let width = canvas.width();
                    let height = canvas.height();
                    let presenter = platform_window.presenter();
                    let coherency = presenter.present_coherency();
                    let present_surface = presenter.present_surface(width, height, dpr);
                    let present_image = presenter.present_image();
                    let prepared_damage = self.present_damage_tracker.prepare(
                        coherency,
                        present_surface,
                        present_image,
                        &damage,
                    );
                    let (damage_plan, damage_commit) = prepared_damage.into_parts();
                    // 帧诊断：呈现阶段起点。
                    let present_start = collect_frame_diagnostics.then(Instant::now);
                    match presenter.present(
                        canvas.pixels(),
                        width,
                        height,
                        damage_plan.present_damage,
                    ) {
                        Ok(()) => {
                            self.present_damage_tracker.commit_prepared(damage_commit);
                            engine.external_present_succeeded();
                            // 帧诊断：呈现阶段耗时。
                            present_us =
                                present_start.map_or(Duration::ZERO, |start| start.elapsed());
                            record_present(metrics, outcome_source);
                            self.rendered_first = true;
                            frame_committed = true;
                        }
                        Err(error) => {
                            engine.external_present_failed(error.clone());
                            tracing::error!(
                                "[WindowDriver] external present failed: {}",
                                error.what()
                            );
                            frame_failure = Some(GraphicsFailure::from_error(error));
                            self.rendered_first = false;
                        }
                    }
                }
            }
            RenderOutcome::Idle => {
                record_idle(metrics, outcome_source);
                if need_render {
                    let message = "frame renderer returned Idle while render work was pending";
                    tracing::error!("[WindowDriver] {message}");
                    frame_failure = Some(protocol_failure(message));
                    self.rendered_first = false;
                }
            }
            RenderOutcome::FrameReady(_) => {
                let message = "frame renderer returned FrameReady without final presentation";
                tracing::error!("[WindowDriver] {message}");
                frame_failure = Some(protocol_failure(message));
                self.rendered_first = false;
            }
            RenderOutcome::Failed(error) => {
                report_graphics_frame_failure(&self.diagnostics, &error);
                self.rendered_first = false;
                frame_failure = Some(error);
            }
        }

        if frame_committed {
            semantic_state.mark_presented();
            self.presented_sequence = self.presented_sequence.wrapping_add(1);
            engine.note_presented_at(frame_time);
            sync_graphics_maintenance(active_work, engine);
            self.frame_scheduler
                .presented(frame_time, target_present_time.is_some());
            if let Some(token) = animation_frame_token
                .filter(|token| self.frame_scheduler.outstanding_native_token() == Some(*token))
            {
                if let Err(error) = platform_window.native_frame_presented(token) {
                    self.diagnostics.observe_transient_error(
                        "window_driver",
                        "native frame present notification failed; fallback remains armed", &error);
                }
            }
        } else if let Some(failure) = frame_failure.as_ref() {
            self.cancel_outstanding_native_frame(platform_window);
            if engine.has_terminal_failure() {
                self.frame_scheduler.mark_terminal_failure();
                // 有界恢复序列已放弃：终态失败只报告一次，进入框架诊断
                // 供宿主快照观察；瞬态失败由 RecoveryDriver 继续恢复不打扰报告。
                if !self.terminal_failure_reported {
                    self.terminal_failure_reported = true;
                    debug_mode.report_with_origin(
                        failure.error().clone(),
                        crate::diagnostics::ReportOrigin::framework("graphics", "terminal_failure"),
                    );
                }
            } else {
                self.frame_scheduler.frame_failed(failure, frame_time);
            }
        }

        if frame_committed && self.deferred_show {
            // GPU 路径已在绘制前显示窗口，CPU 外部 presenter 仍在此处执行首次显示。
            if !pre_present_shown {
                if let Err(error) = platform_window.show() {
                    tracing::error!(
                        "[WindowDriver] deferred show after first present failed: {}",
                        error.short_what()
                    );
                    // 首帧后 deferred show 失败意味着窗口可能持续不可见，
                    // 进入框架报告供宿主观察。
                    debug_mode.report_with_origin(
                        error,
                        crate::diagnostics::ReportOrigin::framework("window", "deferred_show"),
                    );
                }
            }
            // 首帧提交成功后再提升窗口层级，保持原有焦点与窗口顺序语义。
            if let Err(error) = platform_window.raise() {
                tracing::warn!(
                    "[WindowDriver] deferred raise after first present failed: {}",
                    error.short_what()
                );
                debug_mode.report_with_origin(
                    error,
                    crate::diagnostics::ReportOrigin::framework("window", "deferred_raise"),
                );
            }
            let elapsed = self
                .started_at
                .and_then(|started| frame_time.checked_duration_since(started))
                .unwrap_or_default();
            tracing::info!(
                "first_present_ms={} (window visibility and first present handshake completed)",
                elapsed.as_millis()
            );
            self.deferred_show = false;
        }

        self.publish_agent_window_availability(semantic_state, platform_window, engine);

        #[cfg(feature = "test-harness")]
        if let Some(snapshot) = semantic_state.snapshot() {
            tree.publish_automation_snapshot(
                snapshot.generation,
                snapshot.revision,
                snapshot.presented_revision,
                &snapshot.nodes,
            );
        }

        if frame_committed
            && (needs_layout || has_layout || need_render)
            && !tree.reset_invalidation_if_revision(invalidation_revision_before_render)
        {
            // The committed frame consumed the old queue, but paint or
            // presentation produced newer state. Exact queue ownership is
            // intentionally not split here: replaying an already-consumed
            // Composite scroll would move retained pixels twice. Clear the
            // mixed queue and settle the latest tree with one conservative
            // full layout/paint opportunity instead.
            tree.reset_invalidation();
            tree.mark_full_frame_dirty();
        }

        // 帧尾续帧：仍有已注册动画时必须按 cadence 门控续帧。本帧动画 tick
        // 留下的 paint 失效若走 immediate，会在每帧结尾把已武装的动画帧
        // 收紧为「立即」，形成全速重绘链；输入等外部唤醒仍由帧入口的
        // arm_visual_request 以 immediate 渲染，不受此处影响。
        let animation_continue = active_work.animation_ids().next().is_some();
        self.arm_visual_request(
            frame_time,
            tree,
            pending_root,
            *reconcile_pending,
            animation_continue,
            false,
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
            self.frame_scheduler.has_due_opportunity(frame_time),
            agent_commands.has_work(),
        );
        if collect_frame_diagnostics {
            // 诊断标量在脏区所有权移交前已保存，启用收集时必然存在。
            let (rendered_full, dirty_area_pct, dirty_rect_count) =
                dirty_diagnostics.unwrap_or((false, 0.0, 0));
            // 帧诊断：活跃动画节点数与其最大 frame 占窗口比例。
            let (anim_count, anim_biggest_pct) =
                animation_diag(active_work, tree, native_width, native_height);
            // 帧诊断：最大 Paint 失效矩形的节点槽位与面积占比。
            let (inval_big_slot, inval_big_pct) = match inval_biggest {
                Some((id, rect)) => {
                    let total = (native_width as f32) * (native_height as f32);
                    let pct = if total > 0.0 {
                        (rect.w * rect.h / total) as f64
                    } else {
                        0.0
                    };
                    (id.slot() as u64, pct)
                }
                None => (0, 0.0),
            };
            // 帧诊断：累计本帧各阶段耗时，每秒输出一次摘要。
            accumulate_frame_diagnostics(
                &mut self.frame_diag,
                debug_mode,
                platform_window.window_id(),
                debug_correlation_id,
                self.presented_sequence,
                tree.tree_version(),
                native_width.max(0) as u32,
                native_height.max(0) as u32,
                frame_start.map_or(Duration::ZERO, |start| start.elapsed()),
                layout_us,
                render_us,
                present_us,
                // GPU 提交阶段耗时（end_frame 提交与 present 等待）。
                submit_us,
                // 使用渲染输入同源的脏区快照，input 已移入渲染管线。
                rendered_full,
                // 脏区面积占比。
                dirty_area_pct,
                // 全幅区域没有离散矩形，其余保留实际条目数量。
                dirty_rect_count,
                // 活跃动画节点数与最大动画 frame 占比。
                anim_count,
                anim_biggest_pct,
                // 失效条目数与最大 Paint 失效节点。
                inval_count,
                inval_big_slot,
                inval_big_pct,
                // 本帧是否执行了协调（整树重建）及其版本跨度。
                reconcile_ran,
                tree.tree_version().saturating_sub(pre_reconcile_version),
                outcome_source,
            );
        }
        WindowFrameResult { did_work: true }
    }
}
