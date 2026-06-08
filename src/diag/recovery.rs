// ============================================================================
// uix-diag/src/recovery.rs — Retry policies, circuit breaker, fallback
// ============================================================================

use crate::diag::error::*;
use crate::diag::result::*;

use std::fmt;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

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
        Self {
            max_retries,
            initial_delay_ms,
            max_delay_ms,
            multiplier,
            jitter_factor,
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
        let mut base = self.initial_delay_ms as f64;
        for _ in 0..attempt {
            base *= self.multiplier;
            if base >= self.max_delay_ms as f64 {
                base = self.max_delay_ms as f64;
                break;
            }
        }

        if self.jitter_factor > 0.0 {
            use rand::RngExt;
            let mut rng: rand::rngs::SmallRng = rand::make_rng();
            let jitter: f64 =
                rng.random_range((1.0 - self.jitter_factor)..(1.0 + self.jitter_factor));
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
// CircuitBreakerState
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
            _ => {
                // Defensive: treat unknown values as Open to trigger recovery logic
                CircuitState::Open
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// CircuitBreaker
// ════════════════════════════════════════════════════════════════════════════

pub struct CircuitBreaker {
    state: AtomicU64, // stored as CircuitState
    failure_count: AtomicUsize,
    success_count: AtomicUsize,
    rejected_count: AtomicUsize,
    threshold: AtomicUsize,
    recovery_timeout_ms: AtomicU64,
    last_failure_time: AtomicU64,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, recovery_timeout: Duration) -> Self {
        Self {
            state: AtomicU64::new(CircuitState::Closed as u64),
            failure_count: AtomicUsize::new(0),
            success_count: AtomicUsize::new(0),
            rejected_count: AtomicUsize::new(0),
            threshold: AtomicUsize::new(failure_threshold),
            recovery_timeout_ms: AtomicU64::new(recovery_timeout.as_millis() as u64),
            last_failure_time: AtomicU64::new(0),
        }
    }

    fn load_state(&self) -> CircuitState {
        let mut raw = self.state.load(Ordering::Acquire);
        if raw == CircuitState::Open as u64 {
            let last_fail = self.last_failure_time.load(Ordering::Relaxed);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let elapsed = now.saturating_sub(last_fail);
            let timeout = self.recovery_timeout_ms.load(Ordering::Relaxed);
            if elapsed >= timeout {
                // Try to transition to HalfOpen
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
            CircuitState::HalfOpen => true,
        }
    }

    pub fn record_success(&self) {
        self.success_count.fetch_add(1, Ordering::Relaxed);
        let _ = self.state.compare_exchange(
            CircuitState::HalfOpen as u64,
            CircuitState::Closed as u64,
            Ordering::AcqRel,
            Ordering::Relaxed,
        );
        self.failure_count.store(0, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        let fails = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_failure_time.store(now, Ordering::Relaxed);
        if fails >= self.threshold.load(Ordering::Relaxed) {
            self.state
                .store(CircuitState::Open as u64, Ordering::Release);
        }
    }

    pub fn reset(&self) {
        self.state
            .store(CircuitState::Closed as u64, Ordering::Release);
        self.failure_count.store(0, Ordering::Relaxed);
        self.rejected_count.store(0, Ordering::Relaxed);
        self.last_failure_time.store(0, Ordering::Relaxed);
    }

    pub fn set_threshold(&self, failures: usize) {
        self.threshold.store(failures, Ordering::Relaxed);
    }

    pub fn threshold(&self) -> usize {
        self.threshold.load(Ordering::Relaxed)
    }

    pub fn set_recovery_timeout(&self, timeout: Duration) {
        self.recovery_timeout_ms
            .store(timeout.as_millis() as u64, Ordering::Relaxed);
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
            self.recovery_timeout_ms.load(Ordering::Relaxed)
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

// ════════════════════════════════════════════════════════════════════════════
// RecoveryHandler
// ════════════════════════════════════════════════════════════════════════════

pub struct RecoveryHandler {
    policy: Box<dyn RetryPolicy>,
    fallback: Option<Box<dyn Fn(&Error) -> Result<(), Error> + Send + Sync>>,
    on_retry: Option<Box<dyn Fn(&Error, usize) + Send + Sync>>,
}

impl RecoveryHandler {
    pub fn new(policy: Box<dyn RetryPolicy>) -> Self {
        Self {
            policy,
            fallback: None,
            on_retry: None,
        }
    }

    pub fn set_fallback<F>(&mut self, fb: F)
    where
        F: Fn(&Error) -> Result<(), Error> + Send + Sync + 'static,
    {
        self.fallback = Some(Box::new(fb));
    }

    pub fn set_on_retry<F>(&mut self, cb: F)
    where
        F: Fn(&Error, usize) + Send + Sync + 'static,
    {
        self.on_retry = Some(Box::new(cb));
    }

    pub fn execute<F>(&self, operation: F) -> Result<(), Error>
    where
        F: Fn() -> Result<(), Error>,
    {
        let mut last_error;
        let mut attempt = 0;

        loop {
            match operation() {
                Result::Ok(v) => return Result::Ok(v),
                Result::Err(e) => {
                    last_error = e;
                }
            }

            if let Some(ref cb) = self.on_retry {
                cb(&last_error, attempt);
            }

            if !self.policy.should_retry(attempt, &last_error) {
                break;
            }

            let d = self.policy.delay(attempt);
            if d > Duration::from_millis(0) {
                std::thread::sleep(d);
            }
            attempt += 1;
        }

        // Try fallback
        if let Some(ref fb) = self.fallback {
            return fb(&last_error);
        }

        Result::Err(last_error)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 自由函数
// ════════════════════════════════════════════════════════════════════════════

pub fn with_recovery(
    policy: Box<dyn RetryPolicy>,
    operation: impl Fn() -> Result<(), Error>,
    fallback: Option<Box<dyn Fn(&Error) -> Result<(), Error> + Send + Sync>>,
) -> Result<(), Error> {
    let mut handler = RecoveryHandler::new(policy);
    if let Some(fb) = fallback {
        handler.set_fallback(move |e| fb(e));
    }
    handler.execute(operation)
}

pub fn retry(
    max_attempts: usize,
    operation: impl Fn() -> Result<(), Error>,
    delay_between_ms: u64,
) -> Result<(), Error> {
    let policy = FixedRetryPolicy::new(
        if max_attempts > 0 {
            max_attempts - 1
        } else {
            0
        },
        delay_between_ms,
    );
    let handler = RecoveryHandler::new(Box::new(policy));
    handler.execute(operation)
}

// ════════════════════════════════════════════════════════════════════════════
// Generic typed recovery
// ════════════════════════════════════════════════════════════════════════════

pub fn with_recovery_typed<T>(
    policy: Box<dyn RetryPolicy>,
    operation: impl Fn() -> Result<T, Error>,
    fallback: Option<Box<dyn Fn(&Error) -> Result<T, Error> + Send + Sync>>,
) -> Result<T, Error> {
    let mut last_error;
    let mut attempt = 0;

    loop {
        match operation() {
            Result::Ok(v) => return Result::Ok(v),
            Result::Err(e) => {
                last_error = e;
            }
        }

        if !policy.should_retry(attempt, &last_error) {
            break;
        }

        let d = policy.delay(attempt);
        if d > Duration::from_millis(0) {
            std::thread::sleep(d);
        }
        attempt += 1;
    }

    if let Some(fb) = fallback {
        return fb(&last_error);
    }

    Result::Err(last_error)
}
