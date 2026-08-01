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
