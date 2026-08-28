use std::sync::Arc;
use std::time::Instant;

pub(crate) trait AppClock: Send + Sync {
    fn now(&self) -> Instant;
}

#[derive(Debug, Default)]
pub(crate) struct SystemClock;

impl AppClock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

pub(crate) fn system_clock() -> Arc<dyn AppClock> {
    Arc::new(SystemClock)
}

/// 取两个可选截止时间中较早的一个；任一为 `None` 时取另一个，均为 `None` 返回 `None`。
pub(crate) fn earliest_deadline(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(deadline), None) | (None, Some(deadline)) => Some(deadline),
        (None, None) => None,
    }
}
