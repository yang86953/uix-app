use crate::app::clock::AppClock;
use crate::tests::common::*;

#[derive(Debug)]
pub(crate) struct TestClock {
    now: std::sync::Mutex<Instant>,
}

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

impl AppClock for TestClock {
    fn now(&self) -> Instant {
        *self.now.lock().unwrap_or_else(|e| e.into_inner())
    }
}
