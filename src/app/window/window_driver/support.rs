use super::*;
// 帧诊断：读取每秒文本布局调用计数。
use crate::draw::resources::font::font_service::take_text_layout_calls;
use crate::diagnostics::ReportOrigin;
// 卡顿阈值环境变量只解析一次。
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowFrameResult {
    pub(crate) did_work: bool,
}

pub(super) fn with_platform_clipboard<R>(
    platform: &mut Option<&mut dyn PlatformSystem>,
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
        // 每个到期工作项开始前都复核树是否仍接受外部回调。
        if !tree.accepts_external_work() {
            // 首个回调使树停止后不得继续派发后续计时器。
            break;
        }
        match *work {
            ActiveWorkKind::Timer(id) => {
                handled_widget_timer |= tree.dispatch_timer_work(id) == EventResult::Handled;
            }
            ActiveWorkKind::AppTimer(id) => {
                app_timers.fire(id, now);
            }
            _ => {}
        }
        // 每个可能执行用户回调的工作项结束后再次复核树状态。
        if !tree.accepts_external_work() {
            // 当前项使树进入停止态时立即结束本轮派发。
            break;
        }
    }
    handled_widget_timer
}

pub(super) fn update_scheduled_and_discovered_animations(
    tree: &mut WidgetTree,
    scheduled_animation_ids: &[NodeId],
    now: Instant,
    dt: f64,
    discover_animation_work: bool,
    updates: &mut Vec<(NodeId, bool)>,
) {
    updates.clear();
    if !scheduled_animation_ids.is_empty() {
        tree.update_animation_nodes_at_into(scheduled_animation_ids, now, dt, updates);
    }
    // 已调度动画可能使树停止，发现阶段前必须阻止第二轮动画调用。
    if !tree.accepts_external_work() {
        // 保留已完成身份的结果供调用方撤销后续帧工作。
        return;
    }
    if discover_animation_work {
        tree.append_animations_except_at(scheduled_animation_ids, now, dt, updates);
    }
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
        // 作用域重建请求属于同一同步声明工作。
        || tree.has_scoped_rebuild_requested()
        || !main_thread_queue.is_empty()
        || tree.has_app_state_focus_requests()
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

pub(super) fn protocol_failure(message: &str) -> GraphicsFailure {
    GraphicsFailure::Other(Error::new(Errc::InvalidState, message))
}

pub(crate) fn graphics_failure_is_error(failure: &GraphicsFailure) -> bool {
    !matches!(failure, GraphicsFailure::Occluded(_))
}

pub(super) fn report_graphics_frame_failure(
    diagnostics: &Diagnostics,
    failure: &GraphicsFailure,
) {
    if graphics_failure_is_error(failure) {
        // 图形帧失败由 RecoveryDriver 有界恢复收敛：瞬态 episode 经冷却
        // 去重观察，恢复放弃的终态由帧驱动边界单独报告。
        diagnostics.observe_transient_error(
            "graphics",
            "graphics frame failed; bounded recovery owns the episode",
            failure.error(),
        );
    } else {
        tracing::debug!("[WindowDriver] graphics surface occluded; waiting for availability",);
    }
}

// 窗口操作失败没有恢复状态机兜底，在最终观察边界直接进入框架报告。
pub(super) fn report_window_operation_error(
    diagnostics: &Diagnostics,
    context: &'static str,
    result: crate::core::Result<()>,
) {
    if let Err(error) = result {
        tracing::warn!("{context}: {}", error.short_what());
        diagnostics.report_with_origin(error, ReportOrigin::framework("window", context));
    }
}

// resize 失败已被 RecoveryDriver 登记并驱动下一帧有界恢复序列；瞬态失败经
// 冷却去重观察，待恢复放弃为 terminal_failure 时才由帧驱动边界统一报告。
pub(super) fn report_graphics_resize_error(
    diagnostics: &Diagnostics,
    context: &'static str,
    result: crate::core::Result<()>,
) -> bool {
    match result {
        Ok(()) => true,
        Err(error) => {
            diagnostics.observe_transient_error("graphics", context, &error);
            false
        }
    }
}

fn engine_logical_extent(engine: &mut dyn RenderTarget) -> Option<(f32, f32)> {
    let (width, height) = engine.logical_extent();
    (width > 0 && height > 0).then_some((width as f32, height as f32))
}

// 将根节点对齐到最新 Surface，并把尺寸变化发布为真正的布局失效。
// 主窗会显式携带 had_layout_event，副窗则依赖本失效进入同一布局事务。
fn sync_root_surface_frame(tree: &mut WidgetTree, width: f32, height: f32) -> bool {
    let Some(root_id) = tree.root_id() else {
        return false;
    };
    let mismatched = tree.get(root_id).is_some_and(|root| {
        let frame = root.frame();
        (frame.w - width).abs() > 0.5 || (frame.h - height).abs() > 0.5
    });
    if !mismatched {
        return false;
    }
    let Some(root) = tree.get_mut(root_id) else {
        return false;
    };
    root.set_frame(Rect::new(0.0, 0.0, width, height));
    tree.tree_version = tree.tree_version.wrapping_add(1);
    tree.push_layout_invalidation(root_id);
    tree.mark_full_frame_dirty();
    true
}

/// 读取平台窗口协议提供的当前逻辑客户区。
pub(crate) fn native_client_logical_extent(platform_window: &dyn PlatformWindow) -> (i32, i32) {
    // OS 查询与尺寸换算都封装在 PlatformWindow 实现中。
    platform_window.client_logical_extent()
}

pub(crate) fn ensure_surface_matches_window(
    tree: &mut WidgetTree,
    engine: &mut dyn RenderTarget,
    native_width: i32,
    native_height: i32,
    diagnostics: &Diagnostics,
) -> bool {
    if native_width <= 0 || native_height <= 0 {
        return false;
    }
    let Some((canvas_width, canvas_height)) = engine_logical_extent(engine) else {
        return false;
    };
    let mut changed = false;
    let engine_smaller_or_uninitialized = canvas_width <= 1.0
        || canvas_height <= 1.0
        || canvas_width + 0.5 < native_width as f32
        || canvas_height + 0.5 < native_height as f32;
    if engine_smaller_or_uninitialized
        && ((canvas_width - native_width as f32).abs() > 0.5
            || (canvas_height - native_height as f32).abs() > 0.5)
    {
        if report_graphics_resize_error(
            diagnostics,
            "window graphics size reconciliation failed",
            engine.resize(native_width, native_height),
        ) {
            changed = true;
        } else {
            tree.mark_full_frame_dirty();
        }
    }
    let Some((engine_width, engine_height)) = engine_logical_extent(engine) else {
        return changed;
    };
    sync_root_surface_frame(tree, engine_width, engine_height);
    changed
}

pub(crate) fn sync_root_frame_exactly_to_engine(
    tree: &mut WidgetTree,
    engine: &mut dyn RenderTarget,
) {
    let Some((width, height)) = engine_logical_extent(engine) else {
        return;
    };
    sync_root_surface_frame(tree, width, height);
}

pub(crate) fn sync_root_frame_to_engine(tree: &mut WidgetTree, engine: &mut dyn RenderTarget) {
    let Some((engine_width, engine_height)) = engine_logical_extent(engine) else {
        return;
    };
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
        sync_root_surface_frame(tree, engine_width, engine_height);
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

pub(super) fn record_paint(metrics: Option<&Cell<RenderMetrics>>) {
    if let Some(metrics) = metrics {
        let mut value = metrics.get();
        value.record_paint();
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

// 帧诊断：统计活跃动画节点数及最大动画节点 frame 占窗口面积比例。
pub(super) fn animation_diag(
    // 接收逐窗活动工作注册表。
    active_work: &ActiveWorkRegistry,
    // 读取动画节点的布局框。
    tree: &WidgetTree,
    // 接收窗口逻辑宽度。
    window_w: i32,
    // 接收窗口逻辑高度。
    window_h: i32,
) -> (u32, f64) {
    // 收集当前活跃动画身份。
    let ids: Vec<_> = active_work.animation_ids().collect();
    // 动画节点数量。
    let count = ids.len() as u32;
    // 窗口面积，用于计算动画 frame 占比。
    let total = (window_w as f32) * (window_h as f32);
    // 追踪最大动画节点 frame 面积。
    let mut biggest = 0.0f32;
    // 遍历全部活跃动画节点取最大 frame。
    for id in ids {
        // 节点可能已随树变化移除，仅统计仍存在的节点。
        if let Some(node) = tree.get(id) {
            let frame = node.frame();
            let area = frame.w * frame.h;
            // 保留最大的动画区域。
            if area > biggest {
                biggest = area;
            }
        }
    }
    // 窗口尺寸有效时返回面积占比，否则记为零。
    if total > 0.0 {
        (count, (biggest / total) as f64)
    } else {
        (count, 0.0)
    }
}

// 帧诊断：读取失效队列统计（条目数、最大 Paint 失效矩形）。
pub(super) fn invalidation_diag(
    // 读取树持有的失效队列。
    tree: &WidgetTree,
) -> (usize, Option<(NodeId, Rect)>) {
    // 在锁内读取统计快照，避免失效中途变化。
    tree.invalidation
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .diag_largest_paint()
}

const DEFAULT_SLOW_FRAME_THRESHOLD: Duration = Duration::from_millis(100);

fn parse_slow_frame_threshold(value: &std::ffi::OsStr) -> Option<Duration> {
    let milliseconds = value.to_str()?.trim().parse::<u64>().ok()?;
    (milliseconds > 0).then(|| Duration::from_millis(milliseconds))
}

// 显式卡顿阈值会在调试覆盖层关闭时单独启用帧诊断。
fn configured_slow_frame_threshold() -> Option<Duration> {
    static THRESHOLD: OnceLock<Option<Duration>> = OnceLock::new();
    *THRESHOLD.get_or_init(|| {
        let value = std::env::var_os("UIX_SLOW_FRAME_MS")?;
        let parsed = parse_slow_frame_threshold(&value);
        if parsed.is_none() {
            tracing::warn!(
                target: "uix::diagnostics",
                debug_event = "invalid_slow_frame_threshold",
                "UIX_SLOW_FRAME_MS must be a positive integer"
            );
        }
        parsed
    })
}

/// 关闭调试且没有显式卡顿阈值时，帧驱动跳过计时与现场扫描。
pub(super) fn frame_diagnostics_enabled(debug_mode: bool) -> bool {
    debug_mode || configured_slow_frame_threshold().is_some()
}

// 卡顿帧判定阈值：调试模式默认 100ms，环境变量可显式覆盖。
fn slow_frame_threshold() -> Duration {
    configured_slow_frame_threshold().unwrap_or(DEFAULT_SLOW_FRAME_THRESHOLD)
}

// 卡顿自动记录：检测单帧超阈值，输出开始/结束/持续中的现场详情。
fn record_slow_frame(
    // 接收窗口持有的诊断统计。
    diag: &mut super::FrameDiagnostics,
    // 保留逐窗身份与触发批次关联身份。
    window_id: WindowId,
    correlation_id: Option<u64>,
    // 接收本帧总耗时。
    frame_us: Duration,
    // 接收本帧各阶段耗时。
    layout_us: Duration,
    render_us: Duration,
    submit_us: Duration,
    present_us: Duration,
    // 接收本帧重绘范围与失效现场。
    dirty_full: bool,
    dirty_area_pct: f64,
    inval_count: usize,
    inval_big_slot: u64,
    inval_big_pct: f64,
    // 接收本帧动画与协调现场。
    anim_count: u32,
    reconcile_ran: bool,
    version_delta: u64,
    source: InvalidationSource,
) {
    // 超过阈值进入或延续卡顿段。
    let slow = frame_us > slow_frame_threshold();
    if slow {
        // 跟踪卡顿段峰值。
        diag.slow_peak = diag.slow_peak.max(frame_us);
        // 首次进入卡顿段时记录开始时刻。
        if !diag.slow_active {
            diag.slow_active = true;
            diag.slow_since = Some(Instant::now());
            // 卡顿开始：立即输出现场详情。
            tracing::warn!(
                target: "uix::diagnostics",
                debug_event = "slow_frame_start",
                window_id = ?window_id,
                correlation_id = correlation_id.unwrap_or(0),
                has_correlation = correlation_id.is_some(),
                frame_ms = frame_us.as_secs_f64() * 1000.0,
                layout_ms = layout_us.as_secs_f64() * 1000.0,
                render_ms = render_us.as_secs_f64() * 1000.0,
                submit_ms = submit_us.as_secs_f64() * 1000.0,
                present_ms = present_us.as_secs_f64() * 1000.0,
                dirty_percent = dirty_area_pct * 100.0,
                dirty_full,
                invalidation_count = inval_count,
                largest_invalidation_slot = inval_big_slot,
                largest_invalidation_percent = inval_big_pct * 100.0,
                animation_count = anim_count,
                reconcile_ran,
                tree_version_delta = version_delta,
                invalidation_source = source.label(),
                "slow frame segment started"
            );
        }
        return;
    }
    // 低于阈值且处于卡顿段：输出结束摘要。
    if diag.slow_active {
        diag.slow_active = false;
        // 计算卡顿段持续时长。
        let duration = diag
            .slow_since
            .take()
            .map(|start| start.elapsed())
            .unwrap_or_default();
        // 取峰值耗时。
        let peak = diag.slow_peak;
        // 复位峰值统计。
        diag.slow_peak = Duration::ZERO;
        // 卡顿结束：报告持续时长与峰值。
        tracing::warn!(
            target: "uix::diagnostics",
            debug_event = "slow_frame_end",
            window_id = ?window_id,
            correlation_id = correlation_id.unwrap_or(0),
            has_correlation = correlation_id.is_some(),
            duration_seconds = duration.as_secs_f64(),
            peak_ms = peak.as_secs_f64() * 1000.0,
            "slow frame segment ended"
        );
    }
}

// 累计一帧的阶段耗时，并每隔一秒输出一次性能摘要（定位卡顿用）。
pub(super) fn accumulate_frame_diagnostics(
    // 接收窗口持有的累计诊断统计。
    diag: &mut super::FrameDiagnostics,
    // 接收统一诊断 System，写入有界复现环形缓冲。
    diagnostics: &Diagnostics,
    // 接收窗口与触发批次身份，关联输入和最终帧。
    window_id: WindowId,
    correlation_id: Option<u64>,
    // 接收已成功呈现的逐窗口帧序号。
    frame_sequence: u64,
    // 接收当前场景树版本。
    tree_version: u64,
    // 接收原生客户区尺寸，形成逐窗口数值快照。
    width: u32,
    height: u32,
    // 接收本帧总耗时。
    frame_us: Duration,
    // 接收本帧布局阶段耗时。
    layout_us: Duration,
    // 接收本帧渲染阶段耗时。
    render_us: Duration,
    // 接收本帧呈现阶段耗时。
    present_us: Duration,
    // 接收本帧 GPU 提交阶段耗时。
    submit_us: Duration,
    // 接收本帧是否全幅重绘。
    dirty_full: bool,
    // 接收本帧脏区面积占窗口面积比例（0~1）。
    dirty_area_pct: f64,
    // 接收本帧实际脏矩形数量。
    dirty_rect_count: usize,
    // 接收本帧活跃动画节点数。
    anim_count: u32,
    // 接收本帧最大动画节点 frame 占窗口面积比例（0~1）。
    anim_biggest_pct: f64,
    // 接收本帧失效条目数。
    inval_count: usize,
    // 接收本帧最大 Paint 失效节点的槽位。
    inval_big_slot: u64,
    // 接收本帧最大 Paint 失效矩形占窗口面积比例（0~1）。
    inval_big_pct: f64,
    // 接收本帧是否执行了整树协调。
    reconcile_ran: bool,
    // 接收本帧协调导致的树版本增量。
    version_delta: u64,
    // 接收本帧失效来源标签。
    source: InvalidationSource,
) {
    // 只用成功提交帧更新 HUD；Idle 或失败尝试不得伪装成“已呈现帧”。
    if source != InvalidationSource::None {
        diag.last_frame = Some(DebugFrameSnapshot {
            frame_sequence,
            correlation_id,
            tree_version,
            tree_version_delta: version_delta,
            frame_time: frame_us,
            layout_time: layout_us,
            render_time: render_us,
            submit_time: submit_us,
            present_time: present_us,
            dirty_full,
            dirty_rect_count,
            dirty_area_ratio: dirty_area_pct.clamp(0.0, 1.0),
            invalidation_count: inval_count,
            largest_invalidation_slot: (inval_count > 0).then_some(inval_big_slot),
            largest_invalidation_ratio: inval_big_pct.clamp(0.0, 1.0),
            animation_count: anim_count,
            reconcile_ran,
            invalidation_source: source,
        });
    }
    diagnostics.record_debug_frame(
        window_id,
        correlation_id,
        width,
        height,
        frame_us,
        layout_us,
        render_us,
        submit_us,
        present_us,
        dirty_full,
        dirty_area_pct,
        anim_count,
        inval_count,
        reconcile_ran,
        version_delta,
        source.label(),
    );
    // 卡顿自动记录：先检测本帧是否超阈值并输出开始/结束现场。
    record_slow_frame(
        diag,
        window_id,
        correlation_id,
        frame_us,
        layout_us,
        render_us,
        submit_us,
        present_us,
        dirty_full,
        dirty_area_pct,
        inval_count,
        inval_big_slot,
        inval_big_pct,
        anim_count,
        reconcile_ran,
        version_delta,
        source,
    );
    // 只累计实际渲染帧：Idle 帧无呈现工作，混入会稀释阶段均值，也会把
    // 吞吐式 fps 虚高成远超呈现节奏的读数（HUD 已遵循同一口径）。
    let rendered = source != InvalidationSource::None;
    if rendered {
        // 累计帧数，饱和加法避免极端时间戳溢出。
        diag.frames = diag.frames.saturating_add(1);
        // 累计总耗时。
        diag.frame_sum = diag.frame_sum.saturating_add(frame_us);
        // 跟踪单帧最大耗时。
        diag.frame_max = diag.frame_max.max(frame_us);
        // 累计各阶段耗时。
        diag.layout_sum = diag.layout_sum.saturating_add(layout_us);
        diag.render_sum = diag.render_sum.saturating_add(render_us);
        diag.present_sum = diag.present_sum.saturating_add(present_us);
        // 累计 GPU 提交阶段耗时。
        diag.submit_sum = diag.submit_sum.saturating_add(submit_us);
        // 累计全幅重绘帧数。
        if dirty_full {
            diag.full_frames = diag.full_frames.saturating_add(1);
        }
        // 累计脏区面积比例。
        diag.dirty_area_sum += dirty_area_pct.clamp(0.0, 1.0);
    }
    // 每满一秒输出一次摘要，避免日志刷屏。
    if diag.last_report.elapsed() < Duration::from_secs(1) {
        return;
    }
    // 摘要窗口的真实墙钟时长：帧率按渲染帧数除以实际经过时间计算，
    // 反映呈现节奏而非忙等吞吐。
    let elapsed_seconds = diag.last_report.elapsed().as_secs_f64().max(0.001);
    // 帧数下限保护，避免除零。
    let frames = diag.frames.max(1);
    // 计算阶段平均耗时（毫秒）。
    let avg = |sum: Duration| sum.as_secs_f64() * 1000.0 / frames as f64;
    let avg_frame_ms = avg(diag.frame_sum).max(0.001);
    // 输出每秒性能摘要，供复测对照卡顿场景。
    // 读取并清零文本布局调用计数，得到本秒实际 shaping 次数。
    let text_layout_calls = take_text_layout_calls();
    tracing::info!(
        target: "uix::diagnostics",
        debug_event = "frame_summary",
        window_id = ?window_id,
        correlation_id = correlation_id.unwrap_or(0),
        has_correlation = correlation_id.is_some(),
        fps = diag.frames as f64 / elapsed_seconds,
        rendered_frames = diag.frames,
        average_frame_ms = avg_frame_ms,
        maximum_frame_ms = diag.frame_max.as_secs_f64() * 1000.0,
        average_layout_ms = avg(diag.layout_sum),
        average_render_ms = avg(diag.render_sum),
        average_submit_ms = avg(diag.submit_sum),
        average_present_ms = avg(diag.present_sum),
        full_frame_percent = diag.full_frames * 100 / frames,
        dirty_percent = diag.dirty_area_sum * 100.0 / frames as f64,
        text_layout_calls,
        animation_count = anim_count,
        largest_animation_percent = anim_biggest_pct * 100.0,
        invalidation_count = inval_count,
        largest_invalidation_slot = inval_big_slot,
        largest_invalidation_percent = inval_big_pct * 100.0,
        reconcile_ran,
        tree_version_delta = version_delta,
        invalidation_source = source.label(),
        "frame diagnostics summary"
    );
    // 输出后清空统计，只保留下一次输出的计时起点与卡顿段状态。
    *diag = super::FrameDiagnostics {
        // HUD 始终保留刚完成帧，不随一秒累计窗口清零。
        last_frame: diag.last_frame,
        last_report: Instant::now(),
        // 保留卡顿段的进行中状态与峰值，避免摘要重置打断异常记录。
        slow_active: diag.slow_active,
        slow_peak: diag.slow_peak,
        slow_since: diag.slow_since,
        ..Default::default()
    };
}
