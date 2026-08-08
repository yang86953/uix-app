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
            cursor_pos,
            metrics,
            now,
            had_events,
            had_layout_event,
            next_external_deadline,
            on_runtime_tasks,
            on_frame,
        } = context;

        self.started_at.get_or_insert(now);
        self.publish_agent_window_availability(semantic_state, platform_window);

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
                tracing::warn!(
                    "[WindowDriver] fallback native frame cancellation failed: {}",
                    error.short_what()
                );
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
                        tracing::warn!(
                            "[WindowDriver] native frame request failed; fallback remains armed: {}",
                            error.short_what()
                        );
                    }
                }
            }
        }

        let mut reconcile_ran = false;
        if *reconcile_pending {
            let root = pending_root
                .take()
                .or_else(|| view_factory.and_then(ViewFactorySlot::build));
            if let Some(root) = root {
                ViewAdapter::reconcile_nodes(tree, root);
                reconcile_ran = true;
            }
            *reconcile_pending = false;
        }
        if reconcile_ran {
            sync_animation_registrations(active_work, tree, &[]);
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
            self.publish_agent_window_availability(semantic_state, platform_window);
            return WindowFrameResult { did_work: true };
        }
        let surface_corrected =
            ensure_surface_matches_window(tree, engine, native_width, native_height);
        let has_layout = has_layout_work(tree);
        let needs_layout =
            had_layout_event || surface_corrected || !self.rendered_first || has_layout;

        let mut laid_out = false;
        if needs_layout {
            let before_version = tree.tree_version();
            tree.layout();
            record_layout(metrics);
            laid_out = true;

            sync_root_frame_to_engine(tree, engine);
            if let Some(platform) = platform.as_deref_mut() {
                on_frame(tree, engine, platform);
            }
            sync_root_frame_to_engine(tree, engine);

            if tree.tree_version() != before_version {
                tree.layout();
                record_layout(metrics);
                tree.mark_full_frame_dirty();
            }
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
        // 记录 GPU 首帧前已经成功显示的窗口，避免成功后重复调用 show。
        let mut pre_present_shown = false;
        let (outcome, outcome_source) = if !need_render {
            (RenderOutcome::Idle, InvalidationSource::None)
        } else {
            let theme_ref = theme.borrow();
            tree.set_theme_tokens(theme_ref.tokens_arc());
            let scroll_move = tree.scroll_region_moves();
            let hover_pos = debug_mode.get().then(|| cursor_pos.get());
            let metrics_ref = metrics.map(Cell::get);
            // 原生 swapchain 必须在首个 GPU Present 前进入可见状态，避免 DXGI 将隐藏窗口视为 occluded。
            if self.deferred_show && !engine.capabilities().uses_external_presenter() {
                // 只提前执行可见性切换，保留首帧成功后的统一 show/raise 收尾和失败重试状态。
                match platform_window.show() {
                    // 成功显示后仍保留 deferred_show，直到首帧真正提交才消费状态。
                    Ok(()) => pre_present_shown = true,
                    // 显示失败不丢弃 deferred_show，后续首帧提交仍可重试原有边界。
                    Err(error) => tracing::warn!(
                        "[WindowDriver] pre-present show failed; deferred retry remains armed: {}",
                        error.short_what()
                    ),
                }
            }
            let frame_out = self.frame_renderer.render_frame(
                engine,
                tree,
                FrameRenderInput {
                    rendered_first: self.rendered_first,
                    dirty_region: &dirty_region,
                    tree_version: tree.tree_version(),
                    scroll_move,
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
                    let damage_plan = self.present_damage_tracker.plan(
                        coherency,
                        present_surface,
                        present_image,
                        &damage,
                    );
                    match presenter.present(
                        canvas.pixels(),
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
                report_graphics_frame_failure(&error);
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
                    tracing::warn!(
                        "[WindowDriver] native frame present notification failed; fallback remains armed: {}",
                        error.short_what()
                    );
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

        if frame_committed && self.deferred_show {
            // GPU 路径已在绘制前显示窗口，CPU 外部 presenter 仍在此处执行首次显示。
            if !pre_present_shown {
                if let Err(error) = platform_window.show() {
                    tracing::error!(
                        "[WindowDriver] deferred show after first present failed: {}",
                        error.short_what()
                    );
                }
            }
            // 首帧提交成功后再提升窗口层级，保持原有焦点与窗口顺序语义。
            if let Err(error) = platform_window.raise() {
                tracing::warn!(
                    "[WindowDriver] deferred raise after first present failed: {}",
                    error.short_what()
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

