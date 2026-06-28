// ============================================================================
// platform/src/diagnostic/recovery.rs — 恢复执行逻辑
// ============================================================================

use std::time::Duration;
use crate::error::Error;

pub use crate::diagnostic::recovery_policy::*;

// ── 类型别名 ──────────────────────────────────────────────────────────────

pub(crate) type FallbackFn = Box<dyn Fn(&Error) -> Result<(), Error> + Send + Sync>;
pub(crate) type RetryFn = Box<dyn Fn(&Error, usize) + Send + Sync>;
pub(crate) type TypedFallback<T> = Box<dyn Fn(&Error) -> Result<T, Error> + Send + Sync>;

// ════════════════════════════════════════════════════════════════════════════
// RecoveryHandler
// ════════════════════════════════════════════════════════════════════════════

pub struct RecoveryHandler {
    policy: Box<dyn RetryPolicy>,
    fallback: Option<FallbackFn>,
    on_retry: Option<RetryFn>,
}

impl RecoveryHandler {
    pub fn new(policy: Box<dyn RetryPolicy>) -> Self {
        Self { policy, fallback: None, on_retry: None }
    }

    pub fn set_fallback<F>(&mut self, fb: F)
    where F: Fn(&Error) -> Result<(), Error> + Send + Sync + 'static {
        self.fallback = Some(Box::new(fb));
    }

    pub fn set_on_retry<F>(&mut self, cb: F)
    where F: Fn(&Error, usize) + Send + Sync + 'static {
        self.on_retry = Some(Box::new(cb));
    }

    pub fn execute<F>(&self, operation: F) -> Result<(), Error>
    where F: Fn() -> Result<(), Error> {
        let mut last_error;
        let mut attempt = 0;

        loop {
            match operation() {
                Ok(v) => return Ok(v),
                Err(e) => { last_error = e; }
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

        if let Some(ref fb) = self.fallback {
            return fb(&last_error);
        }

        Err(last_error)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 自由函数
// ════════════════════════════════════════════════════════════════════════════

pub fn with_recovery(
    policy: Box<dyn RetryPolicy>,
    operation: impl Fn() -> Result<(), Error>,
    fallback: Option<FallbackFn>,
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
        if max_attempts > 0 { max_attempts - 1 } else { 0 },
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
    fallback: Option<TypedFallback<T>>,
) -> Result<T, Error> {
    let mut last_error;
    let mut attempt = 0;

    loop {
        match operation() {
            Ok(v) => return Ok(v),
            Err(e) => { last_error = e; }
        }

        if !policy.should_retry(attempt, &last_error) { break; }

        let d = policy.delay(attempt);
        if d > Duration::from_millis(0) {
            std::thread::sleep(d);
        }
        attempt += 1;
    }

    if let Some(fb) = fallback {
        return fb(&last_error);
    }

    Err(last_error)
}
