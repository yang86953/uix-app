//! Shared per-window frame driver.
//!
//! The outer application loops own event collection and window lifetime. This
//! type owns the ordered work -> animation -> reconcile -> layout -> paint ->
//! present pipeline so root and secondary windows cannot drift into different
//! frame semantics.

mod availability;
mod support;

use crate::app::queues::active_work_registry::{ActiveWorkKind, ActiveWorkRegistry, TimerId};
use crate::app::queues::app_timer::AppTimerQueue;
use crate::app::queues::main_thread_queue::{MainThreadContext, MainThreadQueue};
use crate::app::queues::window_agent_state::WindowAgentState;
use crate::app::window::frame_scheduler::{FrameScheduler, SurfaceSuspendReason};
use crate::app::window::text_input::sync_window_text_input;
use crate::app::window::window_session::{ViewFactorySlot, WindowLoopState, WindowTextInputState};
use crate::app::window_semantics::WindowSemanticState;
use crate::core::{Errc, Error, Point, PresentDamageTracker, Rect};
use crate::draw::renderer::GraphicsFailure;
use crate::draw::renderer::{FrameRenderInput, InvalidationSource, RenderMetrics, ScenePipeline};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::image::ImageService;
use crate::draw::scene::NodeId;
use crate::draw::target::RenderTarget;
use crate::draw::RenderOutcome;
use crate::native::platform::Platform;
use crate::native::present::PresentTestResult;
use crate::native::windowing::event::{UiEvent, UiEventPayload, UiEventType};
use crate::native::windowing::window::{NativeFrameRequest, PlatformWindow, WindowOcclusionState};
use crate::ui::adapter::ViewAdapter;
use crate::ui::component::clipboard;
use crate::ui::component::widget::WidgetCore;
use crate::ui::theme::Theme;
use crate::ui::{EventResult, WidgetTree};
use std::cell::{Cell, RefCell};
use std::time::Instant;
pub(crate) use support::{
    animation_clock_should_advance, ensure_surface_matches_window, has_invalidation_work,
    native_client_logical_extent, sync_root_frame_exactly_to_engine, sync_root_frame_to_engine,
    WindowFrameResult,
};
use support::{
    dispatch_due_active_work, earliest_deadline, has_layout_work, next_loop_state,
    observe_agent_settle, protocol_failure, record_idle, record_layout, record_present,
    report_graphics_frame_failure, report_graphics_resize_error, report_window_operation_error,
    update_scheduled_and_discovered_animations, with_platform_clipboard,
};
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
    pub(crate) next_external_deadline: Option<Instant>,
    pub(crate) on_runtime_tasks: &'a mut dyn FnMut(&mut dyn Platform, &mut WidgetTree),
    pub(crate) on_frame: &'a dyn Fn(&mut WidgetTree, &mut dyn RenderTarget, &mut dyn Platform),
}

pub(crate) struct WindowDriver {
    frame_renderer: ScenePipeline,
    rendered_first: bool,
    present_damage_tracker: PresentDamageTracker,
    frame_scheduler: FrameScheduler,
    last_frame: Option<Instant>,
    initial_size: (i32, i32),
    deferred_show: bool,
    started_at: Option<Instant>,
    presented_sequence: u64,
    scheduled_animation_ids_scratch: Vec<NodeId>,
    app_timer_deadlines_scratch: Vec<(TimerId, Instant)>,
    app_timer_deadline_revision: Option<u64>,
    due_work_scratch: Vec<ActiveWorkKind>,
}

pub(crate) fn sync_graphics_maintenance(
    active_work: &mut ActiveWorkRegistry,
    engine: &dyn RenderTarget,
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

mod driver;
mod frame;
