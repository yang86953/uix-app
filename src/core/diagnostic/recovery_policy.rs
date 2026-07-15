// ============================================================================
// core/diagnostic/recovery_policy.rs — 恢复策略
//
// 重试策略、断路器、回退策略。
// ============================================================================

use crate::core::error::{Errc, Error};
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

fn duration_millis_saturating(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

// ════════════════════════════════════════════════════════════════════════════
// RetryPolicy trait
// ════════════════════════════════════════════════════════════════════════════

pub trait RetryPolicy: Send + Sync {
    fn should_retry(&self, attempt: usize, last_error: &Error) -> bool;
    fn delay(&self, attempt: usize) -> Duration;
    fn describe(&self) -> String;
}

// ════════════════════════════════════════════════════════════════════════════
// FixedRetryPolicy
// ════════════════════════════════════════════════════════════════════════════

pub struct FixedRetryPolicy {
    max_retries: usize,
    delay_ms: u64,
}

impl FixedRetryPolicy {
    pub fn new(max_retries: usize, delay_ms: u64) -> Self {
        Self {
            max_retries,
            delay_ms,
        }
    }

    pub fn set_max_retries(&mut self, n: usize) {
        self.max_retries = n;
    }
    pub fn max_retries(&self) -> usize {
        self.max_retries
    }
}

impl RetryPolicy for FixedRetryPolicy {
    fn should_retry(&self, attempt: usize, _last_error: &Error) -> bool {
        attempt < self.max_retries
    }

    fn delay(&self, _attempt: usize) -> Duration {
        Duration::from_millis(self.delay_ms)
    }

    fn describe(&self) -> String {
        format!(
            "fixed_retry(max={}, delay={}ms)",
            self.max_retries, self.delay_ms
        )
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 极简 xorshift64 PRNG
// ════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy)]
struct FastRng(u64);

impl FastRng {
    fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self(seed | 1)
    }

    fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 * (1.0 / 9007199254740992.0)
    }

    fn range_f64(&mut self, low: f64, high: f64) -> f64 {
        low + self.next_f64() * (high - low)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ExponentialBackoffRetryPolicy
// ════════════════════════════════════════════════════════════════════════════

pub struct ExponentialBackoffRetryPolicy {
    max_retries: usize,
    initial_delay_ms: u64,
    max_delay_ms: u64,
    multiplier: f64,
    jitter_factor: f64,
}

impl ExponentialBackoffRetryPolicy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        max_retries: usize,
        initial_delay_ms: u64,
        max_delay_ms: u64,
        multiplier: f64,
        jitter_factor: f64,
    ) -> Self {
        let multiplier = if multiplier.is_finite() {
            multiplier.max(1.0)
        } else {
            1.0
        };
        let jitter_factor = if jitter_factor.is_finite() {
            jitter_factor.clamp(0.0, 1.0)
        } else {
            0.0
        };
        Self {
            max_retries,
            initial_delay_ms,
            max_delay_ms,
            multiplier,
            jitter_factor,
        }
    }

    fn base_delay_ms(&self, attempt: usize) -> f64 {
        let initial = self.initial_delay_ms as f64;
        if attempt == 0 {
            return initial;
        }

        let maximum = self.max_delay_ms as f64;
        if initial == 0.0 || initial >= maximum || self.multiplier == 1.0 {
            return initial.min(maximum);
        }

        let scaled = initial * self.multiplier.powf(attempt as f64);
        if scaled.is_finite() {
            scaled.min(maximum)
        } else {
            maximum
        }
    }
}

impl Default for ExponentialBackoffRetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 50,
            max_delay_ms: 10_000,
            multiplier: 2.0,
            jitter_factor: 0.1,
        }
    }
}

impl RetryPolicy for ExponentialBackoffRetryPolicy {
    fn should_retry(&self, attempt: usize, _last_error: &Error) -> bool {
        attempt < self.max_retries
    }

    fn delay(&self, attempt: usize) -> Duration {
        let mut base = self.base_delay_ms(attempt);

        if self.jitter_factor > 0.0 {
            let mut rng = FastRng::new();
            let jitter = rng.range_f64(1.0 - self.jitter_factor, 1.0 + self.jitter_factor);
            base *= jitter;
        }

        let ms = base.max(1.0) as u64;
        Duration::from_millis(ms.min(self.max_delay_ms))
    }

    fn describe(&self) -> String {
        format!(
            "exponential_backoff(max={}, init={}ms, max_delay={}ms, mult={}, jitter={})",
            self.max_retries,
            self.initial_delay_ms,
            self.max_delay_ms,
            self.multiplier,
            self.jitter_factor
        )
    }
}

// ════════════════════════════════════════════════════════════════════════════
// FilteredRetryPolicy
// ════════════════════════════════════════════════════════════════════════════

pub struct FilteredRetryPolicy {
    inner: Box<dyn RetryPolicy>,
    codes: Vec<Errc>,
}

impl FilteredRetryPolicy {
    pub fn new(inner: Box<dyn RetryPolicy>, codes: Vec<Errc>) -> Self {
        Self { inner, codes }
    }
}

impl RetryPolicy for FilteredRetryPolicy {
    fn should_retry(&self, attempt: usize, last_error: &Error) -> bool {
        if !self.codes.contains(&last_error.code()) {
            return false;
        }
        self.inner.should_retry(attempt, last_error)
    }

    fn delay(&self, attempt: usize) -> Duration {
        self.inner.delay(attempt)
    }

    fn describe(&self) -> String {
        format!("filtered({})", self.inner.describe())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// CircuitBreaker
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum CircuitState {
    Closed = 0,
    Open = 1,
    HalfOpen = 2,
}

impl fmt::Display for CircuitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "closed"),
            CircuitState::Open => write!(f, "open"),
            CircuitState::HalfOpen => write!(f, "half_open"),
        }
    }
}

impl From<u64> for CircuitState {
    fn from(v: u64) -> Self {
        match v {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Open,
        }
    }
}

pub struct CircuitBreaker {
    recovery_epoch: Instant,
    state: AtomicU64,
    failure_count: AtomicUsize,
    success_count: AtomicUsize,
    rejected_count: AtomicUsize,
    threshold: AtomicUsize,
    recovery_timeout_ms: AtomicU64,
    last_failure_time: AtomicU64,
    half_open_probe_in_flight: AtomicBool,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, recovery_timeout: Duration) -> Self {
        Self {
            recovery_epoch: Instant::now(),
            state: AtomicU64::new(CircuitState::Closed as u64),
            failure_count: AtomicUsize::new(0),
            success_count: AtomicUsize::new(0),
            rejected_count: AtomicUsize::new(0),
            threshold: AtomicUsize::new(failure_threshold.max(1)),
            recovery_timeout_ms: AtomicU64::new(duration_millis_saturating(recovery_timeout)),
            last_failure_time: AtomicU64::new(0),
            half_open_probe_in_flight: AtomicBool::new(false),
        }
    }

    fn load_state(&self) -> CircuitState {
        let mut raw = self.state.load(Ordering::Acquire);
        if raw == CircuitState::Open as u64 {
            let last_fail = self.last_failure_time.load(Ordering::Relaxed);
            let now = duration_millis_saturating(self.recovery_epoch.elapsed());
            let elapsed = now.saturating_sub(last_fail);
            let timeout = self.recovery_timeout_ms.load(Ordering::Relaxed);
            if elapsed >= timeout {
                match self.state.compare_exchange(
                    CircuitState::Open as u64,
                    CircuitState::HalfOpen as u64,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => raw = CircuitState::HalfOpen as u64,
                    Err(actual) => raw = actual,
                }
            }
        }
        raw.into()
    }

    pub fn state(&self) -> CircuitState {
        self.load_state()
    }

    pub fn try_call(&self) -> bool {
        match self.load_state() {
            CircuitState::Closed => true,
            CircuitState::Open => {
                self.rejected_count.fetch_add(1, Ordering::Relaxed);
                false
            }
            CircuitState::HalfOpen => {
                let admitted = self
                    .half_open_probe_in_flight
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                    .is_ok();
                if !admitted {
                    self.rejected_count.fetch_add(1, Ordering::Relaxed);
                }
                admitted
            }
        }
    }

    pub fn record_success(&self) {
        match self.state.load(Ordering::Acquire).into() {
            CircuitState::Closed => {
                self.success_count.fetch_add(1, Ordering::Relaxed);
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {}
            CircuitState::HalfOpen => {
                if !self.half_open_probe_in_flight.load(Ordering::Acquire) {
                    return;
                }

                if self
                    .state
                    .compare_exchange(
                        CircuitState::HalfOpen as u64,
                        CircuitState::Closed as u64,
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    self.success_count.fetch_add(1, Ordering::Relaxed);
                    self.failure_count.store(0, Ordering::Relaxed);
                }
                self.half_open_probe_in_flight
                    .store(false, Ordering::Release);
            }
        }
    }

    pub fn record_failure(&self) {
        let fails = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        let was_half_open = self.state.load(Ordering::Acquire) == CircuitState::HalfOpen as u64;
        let now = duration_millis_saturating(self.recovery_epoch.elapsed());
        self.last_failure_time.store(now, Ordering::Relaxed);
        if was_half_open || fails >= self.threshold.load(Ordering::Relaxed) {
            self.state
                .store(CircuitState::Open as u64, Ordering::Release);
            self.half_open_probe_in_flight
                .store(false, Ordering::Release);
        }
    }

    pub fn reset(&self) {
        self.state
            .store(CircuitState::Closed as u64, Ordering::Release);
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.rejected_count.store(0, Ordering::Relaxed);
        self.last_failure_time.store(0, Ordering::Relaxed);
        self.half_open_probe_in_flight
            .store(false, Ordering::Release);
    }

    pub fn set_threshold(&self, failures: usize) {
        let threshold = failures.max(1);
        self.threshold.store(threshold, Ordering::Relaxed);
        if self.failure_count.load(Ordering::Relaxed) >= threshold {
            let _ = self.state.compare_exchange(
                CircuitState::Closed as u64,
                CircuitState::Open as u64,
                Ordering::AcqRel,
                Ordering::Relaxed,
            );
        }
    }
    pub fn threshold(&self) -> usize {
        self.threshold.load(Ordering::Relaxed)
    }

    pub fn set_recovery_timeout(&self, timeout: Duration) {
        self.recovery_timeout_ms
            .store(duration_millis_saturating(timeout), Ordering::Relaxed);
    }
    pub fn recovery_timeout(&self) -> Duration {
        Duration::from_millis(self.recovery_timeout_ms.load(Ordering::Relaxed))
    }

    pub fn failure_count(&self) -> usize {
        self.failure_count.load(Ordering::Relaxed)
    }
    pub fn success_count(&self) -> usize {
        self.success_count.load(Ordering::Relaxed)
    }
    pub fn rejected_count(&self) -> usize {
        self.rejected_count.load(Ordering::Relaxed)
    }

    pub fn describe(&self) -> String {
        format!(
            "circuit_breaker(state={}, failures={}/{}, success={}, rejected={}, timeout={}ms)",
            self.load_state(),
            self.failure_count.load(Ordering::Relaxed),
            self.threshold.load(Ordering::Relaxed),
            self.success_count.load(Ordering::Relaxed),
            self.rejected_count.load(Ordering::Relaxed),
            self.recovery_timeout_ms.load(Ordering::Relaxed),
        )
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, Duration::from_secs(30))
    }
}

// ════════════════════════════════════════════════════════════════════════════
// RecoveryAction
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecoveryAction {
    Retry,
    Fallback,
    Abort,
    Skip,
}
