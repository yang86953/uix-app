//! 共享的逐窗口帧驱动。
//!
//! 外层应用循环负责事件收集与窗口生命周期。本类型持有有序的
//! work → animation → reconcile → layout → paint → present 管线，
//! 保证主窗口与副窗口不会漂移成不同的帧语义。

mod availability;
mod support;

use crate::app::queues::active_work_registry::{ActiveWorkKind, ActiveWorkRegistry, TimerId};
use crate::app::queues::app_timer::AppTimerQueue;
use crate::app::queues::main_thread_queue::{MainThreadContext, MainThreadQueue};
use crate::app::queues::window_agent_state::WindowAgentState;
use crate::app::window::frame_scheduler::{FrameScheduler, SurfaceSuspendReason};
use crate::app::window::text_input::sync_window_text_input;
use crate::app::window::window_agent_ops::PlatformWindowAgentOps;
use crate::app::window::window_session::{ViewFactorySlot, WindowLoopState, WindowTextInputState};
use crate::app::window_semantics::WindowSemanticState;
use crate::core::{Errc, Error, Point, PresentDamageTracker, Rect, WindowId};
use crate::diagnostics::Diagnostics;
use crate::draw::RenderOutcome;
use crate::draw::debug::{DebugFrameSnapshot, DebugHudState};
use crate::draw::renderer::GraphicsFailure;
use crate::draw::renderer::{FrameRenderInput, InvalidationSource, RenderMetrics, ScenePipeline};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::image::ImageService;
use crate::draw::scene::NodeId;
use crate::draw::target::RenderTarget;
use crate::platform::platform::PlatformSystem;
use crate::platform::presentation::PresentTestResult;
use crate::platform::windowing::WindowCapability;
use crate::platform::windowing::event::{UiEvent, UiEventPayload, UiEventType};
use crate::platform::windowing::window::{
    NativeFrameRequest, PlatformWindow, WindowOcclusionState,
};
use crate::ui::adapter::ViewAdapter;
use crate::ui::theme::Theme;
use crate::ui::widget_runtime::clipboard;
use crate::ui::widget_runtime::widget::WidgetCore;
use crate::ui::{EventResult, WidgetTree};
use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};
pub(crate) use support::{
    WindowFrameResult, animation_clock_should_advance, ensure_surface_matches_window,
    has_invalidation_work, native_client_logical_extent, sync_root_frame_exactly_to_engine,
    sync_root_frame_to_engine,
};
use support::{
    accumulate_frame_diagnostics, animation_diag, dispatch_due_active_work,
    frame_diagnostics_enabled, has_layout_work, invalidation_diag, next_loop_state,
    observe_agent_settle, protocol_failure, record_idle, record_layout, record_paint,
    record_present, report_graphics_frame_failure, report_graphics_resize_error,
    report_window_operation_error, update_scheduled_and_discovered_animations,
    with_platform_clipboard,
};
// 截止时间合并是 event-loop 与窗口驱动共用的调度基础设施，权威定义在 queues。
use crate::app::queues::clock::earliest_deadline;
pub(crate) struct WindowFrameContext<'a, 'platform> {
    pub(crate) tree: &'a mut WidgetTree,
    pub(crate) engine: &'a mut dyn RenderTarget,
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
    pub(crate) platform: Option<&'platform mut dyn PlatformSystem>,
    pub(crate) font_service: &'a FontService,
    pub(crate) image_service: &'a ImageService,
    pub(crate) theme: &'a RefCell<Theme>,
    pub(crate) debug_mode: &'a Diagnostics,
    /// 关联触发本帧的输入/运行时批次；空值表示本帧未启用关联追踪。
    pub(crate) debug_correlation_id: Option<u64>,
    /// 可选的逐窗口调试 HUD 交互状态；副窗与测试入口保持固定形态 HUD。
    pub(crate) hud_state: Option<&'a DebugHudState>,
    pub(crate) cursor_pos: &'a Cell<Point>,
    pub(crate) metrics: Option<&'a Cell<RenderMetrics>>,
    pub(crate) now: Instant,
    pub(crate) had_events: bool,
    pub(crate) had_layout_event: bool,
    pub(crate) next_external_deadline: Option<Instant>,
    pub(crate) on_runtime_tasks: &'a mut dyn FnMut(&mut dyn PlatformSystem, &mut WidgetTree),
    pub(crate) on_frame:
        &'a dyn Fn(&mut WidgetTree, &mut dyn RenderTarget, &mut dyn PlatformSystem),
}

// 帧诊断统计：累计帧数与各阶段耗时，供每秒输出一次性能摘要。
#[derive(Debug)]
pub(crate) struct FrameDiagnostics {
    // 上一已完成帧的固定事实，供下一次最终调试 Pass 展示。
    last_frame: Option<DebugFrameSnapshot>,
    // 已累计的渲染帧数。
    frames: u32,
    // 已累计的帧总耗时。
    frame_sum: Duration,
    // 本周期内单帧最大耗时。
    frame_max: Duration,
    // 已累计的布局阶段耗时。
    layout_sum: Duration,
    // 已累计的渲染阶段耗时。
    render_sum: Duration,
    // 已累计的呈现阶段耗时。
    present_sum: Duration,
    // 已累计的 GPU 提交阶段耗时（end_frame 提交与 present 等待）。
    submit_sum: Duration,
    // 本周期内全幅重绘帧数。
    full_frames: u32,
    // 本周期内脏区面积占窗口面积比例的累计（用于估算平均重绘范围）。
    dirty_area_sum: f64,
    // 上次输出摘要的时刻。
    last_report: Instant,
    // 卡顿自动记录：当前是否处于连续超阈值帧段。
    slow_active: bool,
    // 卡顿自动记录：当前卡顿段内的峰值帧耗时。
    slow_peak: Duration,
    // 卡顿自动记录：当前卡顿段的开始时刻。
    slow_since: Option<Instant>,
}

// 默认统计从零开始，计时起点取当前时刻。
impl Default for FrameDiagnostics {
    fn default() -> Self {
        Self {
            last_frame: None,
            frames: 0,
            frame_sum: Duration::ZERO,
            frame_max: Duration::ZERO,
            layout_sum: Duration::ZERO,
            render_sum: Duration::ZERO,
            present_sum: Duration::ZERO,
            submit_sum: Duration::ZERO,
            full_frames: 0,
            dirty_area_sum: 0.0,
            last_report: Instant::now(),
            slow_active: false,
            slow_peak: Duration::ZERO,
            slow_since: None,
        }
    }
}

pub(crate) struct WindowDriver {
    frame_renderer: ScenePipeline,
    rendered_first: bool,
    present_damage_tracker: PresentDamageTracker,
    frame_scheduler: FrameScheduler,
    last_frame: Option<Instant>,
    deferred_show: bool,
    // 最近一次 WindowFocus/WindowBlur 事实；未聚焦窗口忽略全部输入。
    window_focused: bool,
    started_at: Option<Instant>,
    presented_sequence: u64,
    scheduled_animation_ids_scratch: Vec<NodeId>,
    // 逐窗复用动画推进结果，稳态帧不再申请临时输出区。
    animation_updates_scratch: Vec<(NodeId, bool)>,
    // 逐窗复用动画登记快照，交付调度器后继续保留容量。
    animation_registrations_scratch: Vec<(NodeId, Option<Instant>)>,
    app_timer_deadlines_scratch: Vec<(TimerId, Instant)>,
    app_timer_deadline_revision: Option<u64>,
    due_work_scratch: Vec<ActiveWorkKind>,
    // 帧诊断统计，用于定位卡顿时的阶段耗时分布。
    frame_diag: FrameDiagnostics,
}

pub(crate) fn sync_graphics_maintenance(
    active_work: &mut ActiveWorkRegistry,
    engine: &dyn RenderTarget,
) {
    // 引擎声明了空闲资源维护截止时刻时注册维护工作，否则取消注册。
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
    managed_registrations: &mut Vec<(NodeId, Option<Instant>)>,
) {
    // 汇总树内动画源注册：内建源 + 视图过渡源，再与组件动画同步。
    tree.animated_source_registrations_into(managed_registrations);
    tree.extend_view_transition_registrations(managed_registrations);
    active_work.sync_animated_sources(managed_registrations.iter().copied());
    active_work.sync_widget_animations(tree.widget_animation_ids());

    // 逐条应用调用方报告的动画启停变化，未由注册表管理的才单独登记。
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

mod driver;
mod frame;
// 将永久失败后的调度资源收敛隔离为独立组件。
mod frame_fail_stop;
