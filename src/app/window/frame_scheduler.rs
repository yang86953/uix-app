//! Per-window one-shot frame scheduling and surface availability.
//!
//! The scheduler is deliberately independent from native event collection:
//! native callbacks can complete only their exact request token, while
//! platforms without a callback use the same request's monotonic deadline as
//! fallback.

use crate::draw::renderer::GraphicsFailure;
use crate::platform::windowing::event::FrameRequestToken;
use std::time::{Duration, Instant};

const INITIAL_FALLBACK_PERIOD: Duration = Duration::from_nanos(1_000_000_000 / 60);
const MIN_FALLBACK_PERIOD: Duration = Duration::from_millis(4);
const MAX_FALLBACK_PERIOD: Duration = Duration::from_millis(100);
const RECOVERY_BASE_DELAY: Duration = Duration::from_millis(8);
const RECOVERY_MAX_DELAY: Duration = Duration::from_secs(1);
const MAX_RECOVERY_ATTEMPTS: u8 = 8;
const OCCLUSION_PROBE_BASE_DELAY: Duration = Duration::from_millis(100);
const OCCLUSION_PROBE_MAX_DELAY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceSuspendReason {
    Hidden,
    Minimized,
    ZeroExtent,
    Occluded,
    TerminalFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceState {
    Renderable,
    Suspended(SurfaceSuspendReason),
    Recovering { retry_at: Instant, attempt: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameOpportunity {
    fallback_native_token: Option<FrameRequestToken>,
    frame_time: Instant,
    target_present_time: Option<Instant>,
}

impl FrameOpportunity {
    pub(crate) fn frame_time(self) -> Instant {
        self.frame_time
    }

    pub(crate) fn target_present_time(self) -> Option<Instant> {
        self.target_present_time
    }

    pub(crate) fn fallback_token(self) -> Option<FrameRequestToken> {
        self.fallback_native_token
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FrameRequest {
    token: FrameRequestToken,
    native_armed: bool,
    cadence: bool,
    deadline: Instant,
    target_present_time: Option<Instant>,
}

#[derive(Debug)]
pub(crate) struct FrameScheduler {
    generation: u64,
    next_request_id: u64,
    surface_state: SurfaceState,
    outstanding: Option<FrameRequest>,
    ready: Option<FrameOpportunity>,
    fallback_period: Duration,
    last_presented_at: Option<Instant>,
    last_animation_frame: Option<Instant>,
    rebase_animation: bool,
    last_wake_callback_driven: bool,
    recovery_attempt: u8,
    occlusion_probe_at: Option<Instant>,
    occlusion_probe_attempt: u8,
}

impl FrameScheduler {
    pub(crate) fn new(renderable: bool) -> Self {
        Self {
            generation: 1,
            next_request_id: 1,
            surface_state: if renderable {
                SurfaceState::Renderable
            } else {
                SurfaceState::Suspended(SurfaceSuspendReason::ZeroExtent)
            },
            outstanding: None,
            ready: None,
            fallback_period: INITIAL_FALLBACK_PERIOD,
            last_presented_at: None,
            last_animation_frame: None,
            rebase_animation: true,
            last_wake_callback_driven: false,
            recovery_attempt: 0,
            occlusion_probe_at: None,
            occlusion_probe_attempt: 0,
        }
    }

    pub(crate) fn is_renderable(&self) -> bool {
        matches!(self.surface_state, SurfaceState::Renderable)
    }

    fn is_terminal_failure(&self) -> bool {
        self.suspended_reason() == Some(SurfaceSuspendReason::TerminalFailure)
    }

    pub(crate) fn suspended_reason(&self) -> Option<SurfaceSuspendReason> {
        match self.surface_state {
            SurfaceState::Suspended(reason) => Some(reason),
            SurfaceState::Renderable | SurfaceState::Recovering { .. } => None,
        }
    }

    pub(crate) fn has_outstanding_request(&self) -> bool {
        self.outstanding.is_some()
    }

    pub(crate) fn outstanding_native_token(&self) -> Option<FrameRequestToken> {
        self.outstanding
            .filter(|request| request.native_armed)
            .map(|request| request.token)
    }

    pub(crate) fn mark_native_armed(&mut self, token: FrameRequestToken) -> bool {
        let Some(request) = self.outstanding.as_mut() else {
            return false;
        };
        if request.token != token {
            return false;
        }
        request.native_armed = true;
        // 原生回调是首选唤醒源；fallback deadline 只是回调失约时的安全网，
        // 追加一个周期的宽限，避免 deadline 与 vsync 回调竞速抢跑。
        request.deadline += self.fallback_period;
        true
    }

    /// Arms one request. A later request is merged into the existing one;
    /// only an earlier external deadline may tighten it — cadence
    /// continuation must never pull a native-armed request back into a
    /// deadline race with the pending callback.
    pub(crate) fn request_frame(
        &mut self,
        deadline: Instant,
        target_present_time: Option<Instant>,
        cadence: bool,
    ) -> Option<FrameRequestToken> {
        if !self.is_renderable() || self.ready.is_some() {
            return None;
        }
        if let Some(request) = &mut self.outstanding {
            if !cadence && deadline < request.deadline {
                request.deadline = deadline;
                request.target_present_time = target_present_time;
            }
            return None;
        }
        let token = self.allocate_request_token();
        self.outstanding = Some(FrameRequest {
            token,
            native_armed: false,
            cadence,
            deadline,
            target_present_time,
        });
        Some(token)
    }

    pub(crate) fn request_immediate(&mut self, now: Instant) -> Option<FrameRequestToken> {
        self.request_frame(now, None, false)
    }

    pub(crate) fn request_animation_frame(&mut self, after: Instant) -> Option<FrameRequestToken> {
        let target = after + self.fallback_period;
        self.request_frame(target, Some(target), true)
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        match self.surface_state {
            SurfaceState::Renderable => self.outstanding.map(|request| request.deadline),
            SurfaceState::Recovering { retry_at, .. } => Some(retry_at),
            SurfaceState::Suspended(SurfaceSuspendReason::Occluded) => self.occlusion_probe_at,
            SurfaceState::Suspended(_) => None,
        }
    }

    pub(crate) fn has_due_opportunity(&self, now: Instant) -> bool {
        self.ready.is_some()
            || match self.surface_state {
                SurfaceState::Renderable => self
                    .outstanding
                    .is_some_and(|request| request.deadline <= now),
                SurfaceState::Recovering { retry_at, .. } => retry_at <= now,
                SurfaceState::Suspended(_) => false,
            }
    }

    pub(crate) fn has_due_occlusion_probe(&self, now: Instant) -> bool {
        self.surface_state == SurfaceState::Suspended(SurfaceSuspendReason::Occluded)
            && self
                .occlusion_probe_at
                .is_some_and(|deadline| deadline <= now)
    }

    /// Consumes one due idle probe. The caller must either resume the surface,
    /// schedule the next occlusion probe, or enter graphics recovery.
    pub(crate) fn take_due_occlusion_probe(&mut self, now: Instant) -> bool {
        if !self.has_due_occlusion_probe(now) {
            return false;
        }
        self.occlusion_probe_at = None;
        true
    }

    pub(crate) fn occlusion_still_present(&mut self, now: Instant) -> bool {
        if self.surface_state != SurfaceState::Suspended(SurfaceSuspendReason::Occluded) {
            return false;
        }
        self.schedule_occlusion_probe(now);
        true
    }

    /// Converts a due fallback request into an opportunity using the actual
    /// wake timestamp, not the requested deadline.
    pub(crate) fn take_due_opportunity(&mut self, now: Instant) -> Option<FrameOpportunity> {
        if let Some(opportunity) = self.ready.take() {
            return Some(opportunity);
        }
        match self.surface_state {
            SurfaceState::Renderable => {
                let request = self.outstanding?;
                if request.deadline > now {
                    return None;
                }
                self.outstanding = None;
                self.last_wake_callback_driven = false;
                Some(FrameOpportunity {
                    fallback_native_token: request.native_armed.then_some(request.token),
                    frame_time: now,
                    target_present_time: request.target_present_time,
                })
            }
            SurfaceState::Recovering { retry_at, .. } if retry_at <= now => {
                self.surface_state = SurfaceState::Renderable;
                self.last_wake_callback_driven = false;
                Some(FrameOpportunity {
                    fallback_native_token: None,
                    frame_time: now,
                    target_present_time: None,
                })
            }
            SurfaceState::Recovering { .. } | SurfaceState::Suspended(_) => None,
        }
    }

    /// Completes an outstanding native request. Exact tokens prevent a late
    /// callback from consuming a newer request on the same surface generation.
    pub(crate) fn notify_opportunity(
        &mut self,
        token: FrameRequestToken,
        frame_time: Instant,
        target_present_time: Option<Instant>,
    ) -> bool {
        if !self.is_renderable() || self.ready.is_some() {
            return false;
        }
        let Some(request) = self.outstanding else {
            return false;
        };
        if request.token != token
            || !request.native_armed
            || token.surface_generation != self.generation
        {
            return false;
        }
        self.outstanding = None;
        self.last_wake_callback_driven = true;
        self.ready = Some(FrameOpportunity {
            fallback_native_token: None,
            frame_time,
            target_present_time: target_present_time.or(request.target_present_time),
        });
        true
    }

    pub(crate) fn suspend(&mut self, reason: SurfaceSuspendReason) {
        if self.is_terminal_failure() {
            return;
        }
        if self.surface_state == SurfaceState::Suspended(reason) && self.outstanding.is_none() {
            return;
        }
        self.bump_generation();
        self.surface_state = SurfaceState::Suspended(reason);
        self.outstanding = None;
        self.ready = None;
        self.recovery_attempt = 0;
        self.clear_occlusion_probe();
        self.rebase_animation = true;
    }

    /// 生命周期信号只恢复可用性挂起；终态须随窗口 session 显式重建。
    pub(crate) fn resume(&mut self) {
        if self.is_renderable() || self.is_terminal_failure() {
            return;
        }
        self.bump_generation();
        self.surface_state = SurfaceState::Renderable;
        self.outstanding = None;
        self.ready = None;
        self.recovery_attempt = 0;
        self.clear_occlusion_probe();
        self.rebase_animation = true;
    }

    /// Resize/DPR/transform 变化使旧 surface callback 失效，但不得解除终态。
    pub(crate) fn surface_changed(&mut self) {
        if self.is_terminal_failure() {
            return;
        }
        self.bump_generation();
        self.surface_state = SurfaceState::Renderable;
        self.outstanding = None;
        self.ready = None;
        self.recovery_attempt = 0;
        self.clear_occlusion_probe();
        self.rebase_animation = true;
    }

    pub(crate) fn animation_delta(
        &self,
        frame_time: Instant,
        had_registered_animation: bool,
    ) -> Duration {
        if self.rebase_animation || !had_registered_animation {
            return Duration::ZERO;
        }
        self.last_animation_frame
            .and_then(|previous| frame_time.checked_duration_since(previous))
            .unwrap_or_default()
            .min(Duration::from_millis(50))
    }

    pub(crate) fn animation_advanced(&mut self, frame_time: Instant) {
        self.last_animation_frame = Some(frame_time);
        self.rebase_animation = false;
    }

    pub(crate) fn presented(&mut self, frame_time: Instant, cadence_sample: bool) {
        if self.is_terminal_failure() {
            return;
        }
        // 只有原生回调驱动的帧才反映真实呈现节奏：deadline 唤醒（含抢跑
        // 在途回调）的间隔含自身调度相位，采样会把 fallback 周期拖到
        // vsync 之下形成永久竞速，也会在纯 fallback 平台单向棘轮上升。
        if cadence_sample && self.last_wake_callback_driven {
            if let Some(previous) = self.last_presented_at {
                if let Some(observed) = frame_time.checked_duration_since(previous) {
                    if (MIN_FALLBACK_PERIOD..=MAX_FALLBACK_PERIOD).contains(&observed) {
                        self.fallback_period = observed;
                    }
                }
            }
        }
        self.last_presented_at = Some(frame_time);
        self.surface_state = SurfaceState::Renderable;
        self.recovery_attempt = 0;
        self.clear_occlusion_probe();
    }

    /// 将 engine 已确认的恢复终态同步为无 deadline 的吸收态。
    pub(crate) fn mark_terminal_failure(&mut self) {
        if self.is_terminal_failure() {
            return;
        }
        self.bump_generation();
        self.outstanding = None;
        self.ready = None;
        self.rebase_animation = true;
        self.clear_occlusion_probe();
        self.surface_state = SurfaceState::Suspended(SurfaceSuspendReason::TerminalFailure);
        self.recovery_attempt = MAX_RECOVERY_ATTEMPTS;
    }

    /// 保留 dirty 并阻止立即重试；可恢复失败有限退避，终态保持吸收。
    pub(crate) fn frame_failed(&mut self, failure: &GraphicsFailure, now: Instant) {
        if self.is_terminal_failure() {
            return;
        }
        if matches!(failure, GraphicsFailure::OutOfMemory(_)) {
            self.mark_terminal_failure();
            return;
        }
        self.bump_generation();
        self.outstanding = None;
        self.ready = None;
        self.rebase_animation = true;

        // Surface 已在 platform 内完成受控重建，立即用新代际重试且不消耗恢复预算。
        if matches!(failure, GraphicsFailure::SurfaceChanged(_)) {
            self.recovery_attempt = 0;
            self.clear_occlusion_probe();
            self.surface_state = SurfaceState::Recovering {
                retry_at: now,
                attempt: 0,
            };
            return;
        }

        if matches!(failure, GraphicsFailure::Occluded(_)) {
            self.surface_state = SurfaceState::Suspended(SurfaceSuspendReason::Occluded);
            self.recovery_attempt = 0;
            self.clear_occlusion_probe();
            self.schedule_occlusion_probe(now);
            return;
        }

        self.clear_occlusion_probe();

        let attempt = self.recovery_attempt.saturating_add(1);
        self.recovery_attempt = attempt;
        if attempt >= MAX_RECOVERY_ATTEMPTS {
            self.surface_state = SurfaceState::Suspended(SurfaceSuspendReason::TerminalFailure);
            return;
        }

        self.surface_state = SurfaceState::Recovering {
            retry_at: now + recovery_delay(attempt),
            attempt,
        };
    }

    fn schedule_occlusion_probe(&mut self, now: Instant) {
        let attempt = self.occlusion_probe_attempt.saturating_add(1);
        self.occlusion_probe_attempt = attempt;
        self.occlusion_probe_at = Some(now + occlusion_probe_delay(attempt));
    }

    fn clear_occlusion_probe(&mut self) {
        self.occlusion_probe_at = None;
        self.occlusion_probe_attempt = 0;
    }

    fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.generation = 1;
        }
    }

    fn allocate_request_token(&mut self) -> FrameRequestToken {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        if self.next_request_id == 0 {
            self.next_request_id = 1;
        }
        FrameRequestToken::new(self.generation, request_id)
    }
}

fn recovery_delay(attempt: u8) -> Duration {
    let shift = u32::from(attempt.saturating_sub(1)).min(16);
    RECOVERY_BASE_DELAY
        .checked_mul(1u32 << shift)
        .unwrap_or(RECOVERY_MAX_DELAY)
        .min(RECOVERY_MAX_DELAY)
}

fn occlusion_probe_delay(attempt: u8) -> Duration {
    let shift = u32::from(attempt.saturating_sub(1)).min(16);
    OCCLUSION_PROBE_BASE_DELAY
        .checked_mul(1u32 << shift)
        .unwrap_or(OCCLUSION_PROBE_MAX_DELAY)
        .min(OCCLUSION_PROBE_MAX_DELAY)
}

#[cfg(test)]
#[path = "../../../tests-src/app/window/frame_scheduler_tests.rs"]
mod tests;

