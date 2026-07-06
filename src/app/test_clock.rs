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

#[cfg(test)]
#[derive(Debug)]
pub(crate) struct TestClock {
    now: std::sync::Mutex<Instant>,
}

#[cfg(test)]
impl TestClock {
    pub(crate) fn new(now: Instant) -> Arc<Self> {
        Arc::new(Self {
            now: std::sync::Mutex::new(now),
        })
    }

    pub(crate) fn advance(&self, delta: std::time::Duration) {
        let mut now = self.now.lock().unwrap_or_else(|e| e.into_inner());
        *now += delta;
    }
}

#[cfg(test)]
impl AppClock for TestClock {
    fn now(&self) -> Instant {
        *self.now.lock().unwrap_or_else(|e| e.into_inner())
    }
}
