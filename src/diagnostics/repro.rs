//! Diagnostics System 拥有的私有复现清单 Module。
//!
//! 该 Module 只缓存固定 schema 的数值事实与错误码，不保存用户文本、窗口标题、
//! 资源标识、环境变量或本地路径。运行时事件仅在 debug 开启时进入有界环形缓冲；
//! 错误码缓存始终保持很小，以便 panic hook 在不读取 reporting store 的情况下导出。

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::core::{Errc, Error, ErrorSeverity, WindowId};

use super::ReportId;

const REPRO_SCHEMA_VERSION: u32 = 1;
const MAX_EVENTS: usize = 128;
const MAX_ERRORS: usize = 16;
const MAX_CAUSE_CODES: usize = 8;
const MAX_WINDOWS: usize = 8;
const MAX_MANIFEST_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Copy)]
pub(super) enum CaptureReason {
    Manual,
    Panic,
}

impl CaptureReason {
    const fn label(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Panic => "panic",
        }
    }
}

#[derive(Debug, Clone)]
struct ErrorFact {
    id: ReportId,
    code: Errc,
    severity: ErrorSeverity,
    cause_codes: Vec<Errc>,
    causes_truncated: bool,
}

impl ErrorFact {
    fn capture(id: ReportId, error: &Error) -> Self {
        let mut cause_codes = Vec::with_capacity(MAX_CAUSE_CODES);
        let mut current = error.source_error();
        while let Some(cause) = current {
            if cause_codes.len() == MAX_CAUSE_CODES {
                break;
            }
            cause_codes.push(cause.code());
            current = cause.source_error();
        }
        Self {
            id,
            code: error.code(),
            severity: error.severity(),
            causes_truncated: current.is_some(),
            cause_codes,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct WindowFact {
    id: WindowId,
    width: u32,
    height: u32,
    last_sequence: u64,
    last_correlation_id: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
enum EventKind {
    ModeChanged {
        enabled: bool,
    },
    Input {
        event_type: &'static str,
    },
    HoverChanged,
    Frame {
        width: u32,
        height: u32,
        frame_us: u64,
        layout_us: u64,
        render_us: u64,
        submit_us: u64,
        present_us: u64,
        dirty_full: bool,
        dirty_permille: u16,
        animation_count: u32,
        invalidation_count: u64,
        reconcile_ran: bool,
        tree_version_delta: u64,
        invalidation_source: &'static str,
    },
}

#[derive(Debug, Clone, Copy)]
struct EventFact {
    sequence: u64,
    elapsed_us: u64,
    window_id: Option<WindowId>,
    correlation_id: Option<u64>,
    kind: EventKind,
}

#[derive(Debug, Default)]
struct ReproState {
    next_sequence: u64,
    events: VecDeque<EventFact>,
    errors: VecDeque<ErrorFact>,
    windows: Vec<WindowFact>,
    dropped_events: u64,
    dropped_errors: u64,
}

#[derive(Debug, Clone)]
pub(super) struct ReproSnapshot {
    reason: CaptureReason,
    runtime_id: u64,
    captured_at: SystemTime,
    debug_enabled: bool,
    capture_busy: bool,
    events: Vec<EventFact>,
    errors: Vec<ErrorFact>,
    windows: Vec<WindowFact>,
    dropped_events: u64,
    dropped_errors: u64,
}

impl ReproSnapshot {
    pub(super) const fn runtime_id(&self) -> u64 {
        self.runtime_id
    }

    pub(super) const fn captured_at(&self) -> SystemTime {
        self.captured_at
    }
}

/// 持有单个 Diagnostics System 实例的有界复现事实。
pub(super) struct ReproModule {
    started_at: Instant,
    state: Mutex<ReproState>,
}

impl ReproModule {
    pub(super) fn new() -> Self {
        Self {
            started_at: Instant::now(),
            state: Mutex::new(ReproState {
                next_sequence: 1,
                events: VecDeque::with_capacity(MAX_EVENTS),
                errors: VecDeque::with_capacity(MAX_ERRORS),
                windows: Vec::with_capacity(MAX_WINDOWS),
                dropped_events: 0,
                dropped_errors: 0,
            }),
        }
    }

    pub(super) fn record_error(&self, id: ReportId, error: &Error) {
        let fact = ErrorFact::capture(id, error);
        let mut state = self.lock_state();
        if state.errors.len() == MAX_ERRORS {
            state.errors.pop_front();
            state.dropped_errors = state.dropped_errors.saturating_add(1);
        }
        state.errors.push_back(fact);
    }

    pub(super) fn record_mode_changed(&self, enabled: bool) {
        self.record_event(None, None, EventKind::ModeChanged { enabled });
    }

    pub(super) fn record_input(
        &self,
        window_id: WindowId,
        correlation_id: u64,
        event_type: &'static str,
    ) {
        self.record_event(
            Some(window_id),
            Some(correlation_id),
            EventKind::Input { event_type },
        );
    }

    pub(super) fn record_hover_changed(&self, window_id: WindowId, correlation_id: u64) {
        self.record_event(
            Some(window_id),
            Some(correlation_id),
            EventKind::HoverChanged,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn record_frame(
        &self,
        window_id: WindowId,
        correlation_id: Option<u64>,
        width: u32,
        height: u32,
        frame: Duration,
        layout: Duration,
        render: Duration,
        submit: Duration,
        present: Duration,
        dirty_full: bool,
        dirty_area_ratio: f64,
        animation_count: u32,
        invalidation_count: usize,
        reconcile_ran: bool,
        tree_version_delta: u64,
        invalidation_source: &'static str,
    ) {
        self.record_event(
            Some(window_id),
            correlation_id,
            EventKind::Frame {
                width,
                height,
                frame_us: duration_micros(frame),
                layout_us: duration_micros(layout),
                render_us: duration_micros(render),
                submit_us: duration_micros(submit),
                present_us: duration_micros(present),
                dirty_full,
                dirty_permille: ratio_permille(dirty_area_ratio),
                animation_count,
                invalidation_count: u64::try_from(invalidation_count).unwrap_or(u64::MAX),
                reconcile_ran,
                tree_version_delta,
                invalidation_source,
            },
        );
    }

    pub(super) fn capture(
        &self,
        reason: CaptureReason,
        runtime_id: u64,
        debug_enabled: bool,
    ) -> ReproSnapshot {
        self.snapshot_from_state(reason, runtime_id, debug_enabled, self.lock_state(), false)
    }

    /// panic hook 不能等待当前线程可能已持有的锁；忙时仍生成可解释的最小清单。
    pub(super) fn try_capture_for_panic(
        &self,
        runtime_id: u64,
        debug_enabled: bool,
    ) -> ReproSnapshot {
        match self.state.try_lock() {
            Ok(state) => self.snapshot_from_state(
                CaptureReason::Panic,
                runtime_id,
                debug_enabled,
                state,
                false,
            ),
            Err(TryLockError::Poisoned(poisoned)) => self.snapshot_from_state(
                CaptureReason::Panic,
                runtime_id,
                debug_enabled,
                poisoned.into_inner(),
                false,
            ),
            Err(TryLockError::WouldBlock) => ReproSnapshot {
                reason: CaptureReason::Panic,
                runtime_id,
                captured_at: SystemTime::now(),
                debug_enabled,
                capture_busy: true,
                events: Vec::new(),
                errors: Vec::new(),
                windows: Vec::new(),
                dropped_events: 0,
                dropped_errors: 0,
            },
        }
    }

    fn record_event(
        &self,
        window_id: Option<WindowId>,
        correlation_id: Option<u64>,
        kind: EventKind,
    ) {
        let elapsed_us = duration_micros(self.started_at.elapsed());
        let mut state = self.lock_state();
        let sequence = state.next_sequence;
        state.next_sequence = state.next_sequence.saturating_add(1);
        if state.events.len() == MAX_EVENTS {
            state.events.pop_front();
            state.dropped_events = state.dropped_events.saturating_add(1);
        }
        state.events.push_back(EventFact {
            sequence,
            elapsed_us,
            window_id,
            correlation_id,
            kind,
        });
        if let Some(window_id) = window_id {
            let size = match kind {
                EventKind::Frame { width, height, .. } => Some((width, height)),
                _ => None,
            };
            touch_window(
                &mut state.windows,
                window_id,
                sequence,
                correlation_id,
                size,
            );
        }
    }

    fn snapshot_from_state(
        &self,
        reason: CaptureReason,
        runtime_id: u64,
        debug_enabled: bool,
        state: MutexGuard<'_, ReproState>,
        capture_busy: bool,
    ) -> ReproSnapshot {
        ReproSnapshot {
            reason,
            runtime_id,
            captured_at: SystemTime::now(),
            debug_enabled,
            capture_busy,
            events: state.events.iter().copied().collect(),
            errors: state.errors.iter().cloned().collect(),
            windows: state.windows.clone(),
            dropped_events: state.dropped_events,
            dropped_errors: state.dropped_errors,
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, ReproState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn touch_window(
    windows: &mut Vec<WindowFact>,
    id: WindowId,
    sequence: u64,
    correlation_id: Option<u64>,
    size: Option<(u32, u32)>,
) {
    if let Some(window) = windows.iter_mut().find(|window| window.id == id) {
        window.last_sequence = sequence;
        window.last_correlation_id = correlation_id.or(window.last_correlation_id);
        if let Some((width, height)) = size {
            window.width = width;
            window.height = height;
        }
        return;
    }
    if windows.len() == MAX_WINDOWS {
        if let Some((oldest, _)) = windows
            .iter()
            .enumerate()
            .min_by_key(|(_, window)| window.last_sequence)
        {
            windows.remove(oldest);
        }
    }
    let (width, height) = size.unwrap_or((0, 0));
    windows.push(WindowFact {
        id,
        width,
        height,
        last_sequence: sequence,
        last_correlation_id: correlation_id,
    });
}

fn duration_micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

fn ratio_permille(ratio: f64) -> u16 {
    (ratio.clamp(0.0, 1.0) * 1000.0).round() as u16
}

pub(super) fn render(snapshot: &ReproSnapshot) -> String {
    let mut out = String::with_capacity(16 * 1024);
    out.push_str("uix-debug-repro\n");
    push_field(&mut out, "schema_version", REPRO_SCHEMA_VERSION);
    push_field(&mut out, "reason", snapshot.reason.label());
    push_field(&mut out, "runtime_id", snapshot.runtime_id);
    let (seconds, millis) = unix_components(snapshot.captured_at);
    push_field(&mut out, "captured_at_unix_secs", seconds);
    push_field(&mut out, "captured_at_unix_millis", millis);
    push_field(&mut out, "debug_enabled", snapshot.debug_enabled);
    push_field(&mut out, "capture_busy", snapshot.capture_busy);
    push_field(&mut out, "uix_version", env!("CARGO_PKG_VERSION"));
    push_field(&mut out, "target_os", std::env::consts::OS);
    push_field(&mut out, "target_arch", std::env::consts::ARCH);
    push_field(&mut out, "dropped_events", snapshot.dropped_events);
    push_field(&mut out, "dropped_errors", snapshot.dropped_errors);
    push_field(&mut out, "window_count", snapshot.windows.len());
    for (index, window) in snapshot.windows.iter().enumerate() {
        let line = format!(
            "window.{index}=id:{};width:{};height:{};last_seq:{};correlation:{}\n",
            window.id.raw(),
            window.width,
            window.height,
            window.last_sequence,
            optional_u64(window.last_correlation_id),
        );
        if !push_bounded_line(&mut out, &line) {
            break;
        }
    }
    push_field(&mut out, "error_count", snapshot.errors.len());
    for (index, error) in snapshot.errors.iter().enumerate() {
        let mut causes = String::new();
        for (cause_index, code) in error.cause_codes.iter().enumerate() {
            if cause_index > 0 {
                causes.push(',');
            }
            let _ = write!(causes, "{code}");
        }
        let line = format!(
            "error.{index}=id:{};code:{};severity:{};cause_codes:{};causes_truncated:{}\n",
            error.id, error.code, error.severity, causes, error.causes_truncated,
        );
        if !push_bounded_line(&mut out, &line) {
            break;
        }
    }
    // 极端数值仍可能让 128 条完整帧超过 32 KiB；优先保留错误码与最新事件，
    // 并明确写出容量省略数量，不能用截断的半行伪装完整记录。
    let event_lines = snapshot
        .events
        .iter()
        .enumerate()
        .map(|(index, event)| render_event_line(index, event))
        .collect::<Vec<_>>();
    const EVENT_COUNT_FIELDS_BUDGET: usize = 160;
    let event_budget = MAX_MANIFEST_BYTES
        .saturating_sub(out.len())
        .saturating_sub(EVENT_COUNT_FIELDS_BUDGET);
    let mut event_bytes = 0_usize;
    let mut first_rendered = event_lines.len();
    for (index, line) in event_lines.iter().enumerate().rev() {
        if event_bytes.saturating_add(line.len()) > event_budget {
            break;
        }
        event_bytes = event_bytes.saturating_add(line.len());
        first_rendered = index;
    }
    let rendered_events = event_lines.len().saturating_sub(first_rendered);
    push_field(&mut out, "event_count_retained", event_lines.len());
    push_field(&mut out, "event_count_rendered", rendered_events);
    push_field(&mut out, "events_omitted_by_size", first_rendered);
    for line in &event_lines[first_rendered..] {
        let _ = push_bounded_line(&mut out, line);
    }
    out
}

fn render_event_line(index: usize, event: &EventFact) -> String {
    let mut line = format!(
        "event.{index}=seq:{};elapsed_us:{};window:{};correlation:{};",
        event.sequence,
        event.elapsed_us,
        event.window_id.map_or(0, WindowId::raw),
        optional_u64(event.correlation_id),
    );
    match event.kind {
        EventKind::ModeChanged { enabled } => {
            let _ = writeln!(line, "kind:mode_changed;enabled:{enabled}");
        }
        EventKind::Input { event_type } => {
            let _ = writeln!(line, "kind:input;type:{event_type}");
        }
        EventKind::HoverChanged => {
            line.push_str("kind:hover_changed\n");
        }
        EventKind::Frame {
            width,
            height,
            frame_us,
            layout_us,
            render_us,
            submit_us,
            present_us,
            dirty_full,
            dirty_permille,
            animation_count,
            invalidation_count,
            reconcile_ran,
            tree_version_delta,
            invalidation_source,
        } => {
            let _ = writeln!(
                line,
                "kind:frame;size:{width}x{height};frame_us:{frame_us};layout_us:{layout_us};render_us:{render_us};submit_us:{submit_us};present_us:{present_us};dirty_full:{dirty_full};dirty_permille:{dirty_permille};animations:{animation_count};invalidations:{invalidation_count};reconcile:{reconcile_ran};tree_delta:{tree_version_delta};source:{invalidation_source}"
            );
        }
    }
    line
}

fn push_field(out: &mut String, key: &str, value: impl std::fmt::Display) {
    let line = format!("{key}={value}\n");
    let _ = push_bounded_line(out, &line);
}

fn push_bounded_line(out: &mut String, line: &str) -> bool {
    if out.len().saturating_add(line.len()) > MAX_MANIFEST_BYTES {
        return false;
    }
    out.push_str(line);
    true
}

fn optional_u64(value: Option<u64>) -> u64 {
    value.unwrap_or(0)
}

fn unix_components(time: SystemTime) -> (u64, u64) {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (duration.as_secs(), u64::from(duration.subsec_millis())),
        Err(_) => (0, 0),
    }
}
