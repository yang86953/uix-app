use crate::app::active_work_registry::{ActiveWorkKind, ActiveWorkRegistry};
use crate::app::app_timer::AppTimerQueue;
use crate::app::clock::AppClock;
use crate::app::event_loop::event_loop::*;
use crate::app::map_ui_event;
use crate::app::window_driver::{animation_clock_should_advance, sync_root_frame_to_engine};
use crate::app::window_session::{WindowLoopState, WindowSession};
use crate::draw::renderer::RenderMetrics;
use crate::draw::renderer::RenderTarget;
use crate::native::test_harness::{FakePlatform, FakeWindow};
use crate::native::traits::event::{FrameRequestToken, UiEvent, UiEventPayload, UiEventType};
use crate::native::traits::platform::Platform;
use crate::native::traits::present::PresentTestResult;
use crate::native::traits::window::{NativeFrameRequest, PlatformWindow};
use crate::tests::app::test_clock::TestClock;
use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::traits::TokenProvider;
use crate::ui::view::combinators::{button, dynamic_label, label};
use crate::ui::view::{column, row, View, ViewNode};
use crate::ui::widgets::feedback::Tooltip;
use crate::ui::widgets::input::input::Input;
use crate::ui::widgets::Label;
use crate::ui::widgets::{Avatar, Container, Image};
use crate::ui::{Animated, Easing};
use std::any::Any;

struct TestAnimatedWidget {
    remaining_updates: Arc<AtomicUsize>,
    update_calls: Arc<AtomicUsize>,
    recorded_dts: Option<Arc<Mutex<Vec<f64>>>>,
    dirty_rect: Rect,
}

impl TestAnimatedWidget {
    fn new(remaining_updates: Arc<AtomicUsize>, update_calls: Arc<AtomicUsize>) -> Self {
        Self {
            remaining_updates,
            update_calls,
            recorded_dts: None,
            dirty_rect: Rect::new(4.0, 5.0, 6.0, 7.0),
        }
    }

    fn recording(
        remaining_updates: Arc<AtomicUsize>,
        update_calls: Arc<AtomicUsize>,
        recorded_dts: Arc<Mutex<Vec<f64>>>,
    ) -> Self {
        Self {
            remaining_updates,
            update_calls,
            recorded_dts: Some(recorded_dts),
            dirty_rect: Rect::new(4.0, 5.0, 6.0, 7.0),
        }
    }
}

impl WidgetComponent for TestAnimatedWidget {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::RENDER | WidgetCapabilities::ANIMATION)
    }

    fn as_render(&self) -> Option<&dyn WidgetRender> {
        Some(self)
    }

    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        Some(self)
    }

    fn as_animation(&self) -> Option<&dyn WidgetAnimation> {
        Some(self)
    }

    fn as_animation_mut(&mut self) -> Option<&mut dyn WidgetAnimation> {
        Some(self)
    }
}

impl WidgetRender for TestAnimatedWidget {
    fn render(&self, _frame: Rect, _ctx: &mut crate::draw::api::PaintContext, _tree: &WidgetTree) {}
}

impl WidgetAnimation for TestAnimatedWidget {
    fn update_animation(&mut self, dt: f64) -> bool {
        self.update_calls.fetch_add(1, Ordering::Relaxed);
        if let Some(recorded_dts) = &self.recorded_dts {
            recorded_dts
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(dt);
        }
        let previous = self.remaining_updates.fetch_sub(1, Ordering::Relaxed);
        previous > 1
    }

    fn dirty_bounds(&self, _frame: Rect) -> Rect {
        self.dirty_rect
    }
}

struct FailFirstBeginEngine {
    inner: Renderer,
    begin_calls: Arc<AtomicUsize>,
}

impl FailFirstBeginEngine {
    fn new(begin_calls: Arc<AtomicUsize>) -> Self {
        Self {
            inner: Renderer::test(),
            begin_calls,
        }
    }
}

impl RenderTarget for FailFirstBeginEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.inner.try_shutdown()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.resize(width, height)
    }

    fn begin_frame(&mut self, strategy: crate::draw::UpdateStrategy) -> RenderOutcome {
        if self.begin_calls.fetch_add(1, Ordering::Relaxed) == 0 {
            RenderOutcome::Failed(crate::draw::renderer::GraphicsFailure::from_error(
                Error::new(Errc::GraphicsSurfaceLost, "injected first-frame failure"),
            ))
        } else {
            self.inner.begin_frame(strategy)
        }
    }

    fn end_frame(&mut self, damage: &DamageRegion) -> RenderOutcome {
        self.inner.end_frame(damage)
    }

    fn canvas_2d(&mut self) -> &mut dyn crate::draw::Canvas2D {
        self.inner.canvas_2d()
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<crate::draw::command::EncodedFrameExecution, Error> {
        self.inner.try_execute_encoded_frame(encoder)
    }
}

struct OccludeFirstPresentEngine {
    inner: Renderer,
    begin_calls: Arc<AtomicUsize>,
    end_calls: Arc<AtomicUsize>,
    probe_calls: Arc<AtomicUsize>,
}

impl OccludeFirstPresentEngine {
    fn new(
        begin_calls: Arc<AtomicUsize>,
        end_calls: Arc<AtomicUsize>,
        probe_calls: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            inner: Renderer::test(),
            begin_calls,
            end_calls,
            probe_calls,
        }
    }
}

impl RenderTarget for OccludeFirstPresentEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.inner.try_shutdown()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.resize(width, height)
    }

    fn begin_frame(&mut self, strategy: crate::draw::UpdateStrategy) -> RenderOutcome {
        self.begin_calls.fetch_add(1, Ordering::Relaxed);
        self.inner.begin_frame(strategy)
    }

    fn end_frame(&mut self, damage: &DamageRegion) -> RenderOutcome {
        let inner = self.inner.end_frame(damage);
        if self.end_calls.fetch_add(1, Ordering::Relaxed) == 0 {
            RenderOutcome::Failed(crate::draw::renderer::GraphicsFailure::Occluded(
                Error::new(Errc::GraphicsOccluded, "injected first-present occlusion"),
            ))
        } else {
            inner
        }
    }

    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        let call = self.probe_calls.fetch_add(1, Ordering::Relaxed);
        Ok(if call == 0 {
            PresentTestResult::Occluded
        } else {
            PresentTestResult::Presentable
        })
    }

    fn canvas_2d(&mut self) -> &mut dyn crate::draw::Canvas2D {
        self.inner.canvas_2d()
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<crate::draw::command::EncodedFrameExecution, Error> {
        self.inner.try_execute_encoded_frame(encoder)
    }
}

struct IdleResourceMaintenanceEngine {
    inner: Renderer,
    deadline: Option<Instant>,
    present_notes: Arc<AtomicUsize>,
    release_calls: Arc<AtomicUsize>,
    end_calls: Arc<AtomicUsize>,
}

impl IdleResourceMaintenanceEngine {
    fn new(
        present_notes: Arc<AtomicUsize>,
        release_calls: Arc<AtomicUsize>,
        end_calls: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            inner: Renderer::test(),
            deadline: None,
            present_notes,
            release_calls,
            end_calls,
        }
    }
}

impl RenderTarget for IdleResourceMaintenanceEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.inner.try_shutdown()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.resize(width, height)
    }

    fn begin_frame(&mut self, strategy: crate::draw::UpdateStrategy) -> RenderOutcome {
        self.inner.begin_frame(strategy)
    }

    fn end_frame(&mut self, damage: &DamageRegion) -> RenderOutcome {
        self.end_calls.fetch_add(1, Ordering::Relaxed);
        self.inner.end_frame(damage)
    }

    fn note_presented_at(&mut self, now: Instant) {
        self.present_notes.fetch_add(1, Ordering::Relaxed);
        self.deadline = Some(now + Duration::from_millis(250));
    }

    fn idle_resource_deadline(&self) -> Option<Instant> {
        self.deadline
    }

    fn release_idle_resources(&mut self, now: Instant) {
        if self.deadline.is_some_and(|deadline| deadline <= now) {
            self.release_calls.fetch_add(1, Ordering::Relaxed);
            self.deadline = None;
        }
    }

    fn canvas_2d(&mut self) -> &mut dyn crate::draw::Canvas2D {
        self.inner.canvas_2d()
    }

    fn capabilities(&self) -> crate::draw::GraphicsCapabilities {
        self.inner.capabilities()
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<crate::draw::command::EncodedFrameExecution, Error> {
        self.inner.try_execute_encoded_frame(encoder)
    }
}

#[derive(Debug)]
struct SteppingClock {
    now: Mutex<Instant>,
    step: Duration,
}

impl SteppingClock {
    fn new(now: Instant, step: Duration) -> Arc<Self> {
        Arc::new(Self {
            now: Mutex::new(now),
            step,
        })
    }
}

impl AppClock for SteppingClock {
    fn now(&self) -> Instant {
        let mut now = self.now.lock().unwrap_or_else(|error| error.into_inner());
        let current = *now;
        *now += self.step;
        current
    }
}

include!("event_loop_cases_01.rs");
include!("event_loop_cases_02.rs");
include!("event_loop_cases_03.rs");
include!("event_loop_cases_04.rs");
include!("event_loop_cases_05.rs");
