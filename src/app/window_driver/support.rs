use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowFrameResult {
    pub(crate) did_work: bool,
}

pub(super) fn with_platform_clipboard<R>(
    platform: &mut Option<&mut dyn Platform>,
    operation: impl FnOnce() -> R,
) -> R {
    match platform.as_deref_mut() {
        Some(platform) => clipboard::with_clipboard(platform.clipboard(), operation),
        None => operation(),
    }
}

pub(super) fn dispatch_due_active_work(
    tree: &mut WidgetTree,
    app_timers: &AppTimerQueue,
    due_work: &[ActiveWorkKind],
    now: Instant,
) -> bool {
    let mut handled_widget_timer = false;
    for work in due_work {
        match *work {
            ActiveWorkKind::Timer(id) => {
                handled_widget_timer |= tree.dispatch_timer_work(id) == EventResult::Handled;
            }
            ActiveWorkKind::AppTimer(id) => {
                app_timers.fire(id, now);
            }
            _ => {}
        }
    }
    handled_widget_timer
}

pub(super) fn update_scheduled_and_discovered_animations(
    tree: &mut WidgetTree,
    scheduled_animation_ids: &[NodeId],
    dt: f64,
    discover_animation_work: bool,
) -> Vec<(NodeId, bool)> {
    let mut updates = if scheduled_animation_ids.is_empty() {
        Vec::new()
    } else {
        tree.update_animation_nodes(scheduled_animation_ids.iter().copied(), dt)
    };
    if discover_animation_work {
        updates.extend(tree.update_animations_except(scheduled_animation_ids.to_vec(), dt));
    }
    updates
}

pub(crate) fn animation_clock_should_advance(
    had_registered_animation: bool,
    animation_updates: &[(NodeId, bool)],
) -> bool {
    had_registered_animation
        || animation_updates
            .iter()
            .any(|(_, still_active)| *still_active)
}

pub(crate) fn has_invalidation_work(tree: &WidgetTree) -> bool {
    let invalidation = tree
        .invalidation
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    invalidation.has_paint_or_composite() || invalidation.has_layout()
}

pub(super) fn has_layout_work(tree: &WidgetTree) -> bool {
    tree.invalidation
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .has_layout()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn observe_agent_settle(
    agent_commands: &mut WindowAgentState,
    semantic_state: &mut WindowSemanticState,
    tree: &WidgetTree,
    main_thread_queue: &MainThreadQueue,
    pending_root: &Option<crate::ui::view::ViewNode>,
    reconcile_pending: bool,
    layout_work_pending: bool,
    refresh_if_settled: bool,
) {
    if !agent_commands.has_in_flight() {
        return;
    }
    let synchronous_work_pending = pending_root.is_some()
        || reconcile_pending
        || tree.has_reconcile_requested()
        || !main_thread_queue.is_empty()
        || tree.has_app_state_semantic_events()
        || tree.has_pending_effects()
        || layout_work_pending;
    if !synchronous_work_pending && refresh_if_settled {
        semantic_state.refresh(tree);
    }
    agent_commands.finish_or_defer(semantic_state, synchronous_work_pending);
}

pub(super) fn next_loop_state(
    tree: &WidgetTree,
    active_work: &ActiveWorkRegistry,
    registered_deadline: Option<Instant>,
    surface_renderable: bool,
    frame_due: bool,
    agent_work_pending: bool,
) -> WindowLoopState {
    if agent_work_pending || frame_due || (surface_renderable && has_invalidation_work(tree)) {
        WindowLoopState::Active
    } else if !active_work.is_empty() || registered_deadline.is_some() {
        WindowLoopState::RegisteredActive
    } else {
        WindowLoopState::DeepIdle
    }
}

pub(super) fn earliest_deadline(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(deadline), None) | (None, Some(deadline)) => Some(deadline),
        (None, None) => None,
    }
}

pub(super) fn protocol_failure(message: &str) -> GraphicsFailure {
    GraphicsFailure::Other(Error::new(Errc::InvalidState, message))
}

pub(crate) fn graphics_failure_is_error(failure: &GraphicsFailure) -> bool {
    !matches!(failure, GraphicsFailure::Occluded(_))
}

pub(super) fn report_graphics_frame_failure(failure: &GraphicsFailure) {
    if graphics_failure_is_error(failure) {
        crate::core::log::error_fn(format!(
            "[WindowDriver] graphics frame failed: {}",
            failure.error().short_what()
        ));
    } else {
        crate::core::log::debug_fn(
            "[WindowDriver] graphics surface occluded; waiting for availability",
        );
    }
}

pub(super) fn report_window_operation_error(context: &str, result: crate::core::Result<()>) {
    if let Err(error) = result {
        crate::core::log::warn_fn(format!("{context}: {}", error.short_what()));
    }
}

pub(super) fn report_graphics_resize_error(context: &str, result: crate::core::Result<()>) -> bool {
    match result {
        Ok(()) => true,
        Err(error) => {
            crate::core::log::warn_fn(format!("{context}: {}", error.short_what()));
            false
        }
    }
}

pub(super) fn ensure_surface_matches_window(
    tree: &mut WidgetTree,
    engine: &mut dyn GraphicsEngine,
    native_width: i32,
    native_height: i32,
) -> bool {
    if native_width <= 0 || native_height <= 0 {
        return false;
    }
    let (canvas_width, canvas_height) = {
        let canvas = engine.canvas_2d();
        (canvas.width(), canvas.height())
    };
    let mut changed = false;
    if canvas_width != native_width || canvas_height != native_height {
        if !report_graphics_resize_error(
            "window graphics size reconciliation failed",
            engine.resize(native_width, native_height),
        ) {
            tree.mark_full_frame_dirty();
            return false;
        }
        changed = true;
    }
    let (engine_width, engine_height) = {
        let canvas = engine.canvas_2d();
        (canvas.width() as f32, canvas.height() as f32)
    };
    if engine_width <= 0.0 || engine_height <= 0.0 {
        return false;
    }
    let root_mismatch = tree
        .root_id()
        .and_then(|root_id| tree.get(root_id))
        .is_some_and(|root| {
            let frame = root.frame();
            (frame.w - engine_width).abs() > 0.5 || (frame.h - engine_height).abs() > 0.5
        });
    if root_mismatch {
        if let Some(root_id) = tree.root_id() {
            if let Some(root) = tree.get_mut(root_id) {
                root.set_frame(Rect::new(0.0, 0.0, engine_width, engine_height));
            }
        }
        tree.tree_version = tree.tree_version.wrapping_add(1);
        tree.mark_full_frame_dirty();
        changed = true;
    }
    changed
}

pub(super) fn sync_root_frame_exactly_to_engine(
    tree: &mut WidgetTree,
    engine: &mut dyn GraphicsEngine,
) {
    let (width, height) = {
        let canvas = engine.canvas_2d();
        (canvas.width() as f32, canvas.height() as f32)
    };
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    if let Some(root_id) = tree.root_id() {
        let mismatched = tree.get(root_id).is_some_and(|root| {
            let frame = root.frame();
            (frame.w - width).abs() > 0.5 || (frame.h - height).abs() > 0.5
        });
        if mismatched {
            if let Some(root) = tree.get_mut(root_id) {
                root.set_frame(Rect::new(0.0, 0.0, width, height));
            }
            tree.tree_version = tree.tree_version.wrapping_add(1);
        }
    }
}

pub(crate) fn sync_root_frame_to_engine(tree: &mut WidgetTree, engine: &mut dyn GraphicsEngine) {
    let (engine_width, engine_height) = {
        let canvas = engine.canvas_2d();
        (canvas.width() as f32, canvas.height() as f32)
    };
    if engine_width <= 0.0 || engine_height <= 0.0 {
        return;
    }
    let need_sync = tree
        .root_id()
        .and_then(|root_id| tree.get(root_id))
        .is_some_and(|root| {
            let frame = root.frame();
            let bootstrap = frame.w <= 1.0 || frame.h <= 1.0;
            let engine_larger = engine_width > frame.w + 0.5 || engine_height > frame.h + 0.5;
            bootstrap || engine_larger
        });
    if need_sync {
        if let Some(root_id) = tree.root_id() {
            if let Some(root) = tree.get_mut(root_id) {
                root.set_frame(Rect::new(0.0, 0.0, engine_width, engine_height));
            }
        }
        tree.tree_version = tree.tree_version.wrapping_add(1);
        tree.mark_full_frame_dirty();
        tree.layout();
    }
}

pub(super) fn record_layout(metrics: Option<&Cell<RenderMetrics>>) {
    if let Some(metrics) = metrics {
        let mut value = metrics.get();
        value.record_layout();
        metrics.set(value);
    }
}

pub(super) fn record_present(metrics: Option<&Cell<RenderMetrics>>, source: InvalidationSource) {
    if let Some(metrics) = metrics {
        let mut value = metrics.get();
        value.record_present(source);
        metrics.set(value);
    }
}

pub(super) fn record_idle(metrics: Option<&Cell<RenderMetrics>>, source: InvalidationSource) {
    if let Some(metrics) = metrics {
        let mut value = metrics.get();
        value.record_idle_with_source(source);
        metrics.set(value);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn log_frame_metrics(
    frame_us: u128,
    input_us: u128,
    reconcile_us: u128,
    layout_us: u128,
    paint_us: u128,
    present_us: u128,
    had_events: bool,
    reconcile_ran: bool,
    layout_calls: u32,
    dirty_full: bool,
    paint_probe: crate::core::perf_probe::PaintProbeSample,
    present_probe: crate::core::perf_probe::PresentProbeSample,
) {
    if crate::core::perf_probe::perf_probe_enabled() {
        crate::core::log::info_fn(format!(
            "frame_us={} input={} reconcile={} layout={} paint_cpu={} present={} events={} reconcile={} layouts={} dirty_full={} strategy_full={} pixels={} layer_build={} record={} execute={} end_frame={} pic_raster={} pic_blit={} pics={} pic_px={} widgets={} text_us={} texts={} cpu_flush={} flushes={} upload_copy={} fence_wait={} submit_present={} present_skipped={}",
            frame_us,
            input_us,
            reconcile_us,
            layout_us,
            paint_us,
            present_us,
            u8::from(had_events),
            u8::from(reconcile_ran),
            layout_calls,
            u8::from(dirty_full),
            paint_probe.strategy_full,
            present_probe.pixels,
            paint_probe.layer_build_us,
            paint_probe.record_us,
            paint_probe.execute_us,
            paint_probe.end_frame_us,
            paint_probe.picture_raster_us,
            paint_probe.picture_blit_us,
            paint_probe.pictures_rasterized,
            paint_probe.picture_pixels,
            paint_probe.widgets_painted,
            paint_probe.text_us,
            paint_probe.text_draws,
            paint_probe.cpu_flush_us,
            paint_probe.cpu_flushes,
            present_probe.upload_copy_us,
            present_probe.fence_wait_us,
            present_probe.submit_present_us,
            present_probe.skipped,
        ));
    } else {
        crate::core::log::info_fn(format!(
            "frame_us={} input={} reconcile={} layout={} paint_cpu={} present={} events={} reconcile={} layouts={} dirty_full={}",
            frame_us,
            input_us,
            reconcile_us,
            layout_us,
            paint_us,
            present_us,
            u8::from(had_events),
            u8::from(reconcile_ran),
            layout_calls,
            u8::from(dirty_full),
        ));
    }
}
