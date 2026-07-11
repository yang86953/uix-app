use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, Weak,
};
use std::time::{Duration, Instant};

use crate::app::active_work_registry::TimerId;
use crate::app::test_clock::{system_clock, AppClock};

#[derive(Clone)]
pub(crate) struct AppTimerQueue {
    inner: Arc<Mutex<AppTimerQueueInner>>,
    clock: Arc<dyn AppClock>,
}

struct AppTimerQueueInner {
    next_id: TimerId,
    cancellation_epoch: u64,
    entries: BTreeMap<TimerId, AppTimerEntry>,
}

impl Default for AppTimerQueueInner {
    fn default() -> Self {
        Self {
            next_id: 1,
            cancellation_epoch: 0,
            entries: BTreeMap::new(),
        }
    }
}

struct AppTimerEntry {
    deadline: Instant,
    interval: Option<Duration>,
    callback: Box<dyn FnMut() + Send>,
    cancellation_epoch: u64,
    active: Arc<AtomicBool>,
}

pub struct TimerHandle {
    id: TimerId,
    queue: Weak<Mutex<AppTimerQueueInner>>,
    active: Arc<AtomicBool>,
    cancelled: bool,
    /// When true, [`Drop`] does not cancel — timer lives until [`Self::cancel`] or session teardown.
    detach_on_drop: bool,
}

impl AppTimerQueue {
    const MIN_INTERVAL: Duration = Duration::from_millis(1);

    pub(crate) fn new() -> Self {
        Self::with_clock(system_clock())
    }

    pub(crate) fn with_clock(clock: Arc<dyn AppClock>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AppTimerQueueInner::default())),
            clock,
        }
    }

    pub(crate) fn run_after<F>(&self, delay: Duration, f: F) -> TimerHandle
    where
        F: FnOnce() + Send + 'static,
    {
        let mut f = Some(f);
        self.insert(delay, None, move || {
            if let Some(f) = f.take() {
                f();
            }
        })
    }

    pub(crate) fn run_interval<F>(&self, interval: Duration, f: F) -> TimerHandle
    where
        F: FnMut() + Send + 'static,
    {
        let interval = interval.max(Self::MIN_INTERVAL);
        self.insert(interval, Some(interval), f)
    }

    pub(crate) fn deadlines(&self) -> Vec<(TimerId, Instant)> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entries
            .iter()
            .map(|(&id, entry)| (id, entry.deadline))
            .collect()
    }

    pub(crate) fn cancel_all(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.cancellation_epoch = inner.cancellation_epoch.wrapping_add(1);
        for entry in inner.entries.values() {
            entry.active.store(false, Ordering::Release);
        }
        inner.entries.clear();
    }

    pub(crate) fn fire(&self, id: TimerId, now: Instant) -> bool {
        let mut entry = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.entries.remove(&id)
        };

        let Some(mut entry) = entry.take() else {
            return false;
        };

        (entry.callback)();

        if let Some(interval) = entry.interval {
            entry.deadline = now + interval;
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if entry.active.load(Ordering::Acquire)
                && entry.cancellation_epoch == inner.cancellation_epoch
            {
                inner.entries.insert(id, entry);
            }
        }

        true
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entries
            .len()
    }

    fn insert<F>(&self, delay: Duration, interval: Option<Duration>, f: F) -> TimerHandle
    where
        F: FnMut() + Send + 'static,
    {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let id = inner.next_id;
        inner.next_id = inner.next_id.wrapping_add(1).max(1);
        let cancellation_epoch = inner.cancellation_epoch;
        let active = Arc::new(AtomicBool::new(true));
        inner.entries.insert(
            id,
            AppTimerEntry {
                deadline: self.clock.now() + delay,
                interval,
                callback: Box::new(f),
                cancellation_epoch,
                active: active.clone(),
            },
        );
        TimerHandle {
            id,
            queue: Arc::downgrade(&self.inner),
            active,
            cancelled: false,
            detach_on_drop: false,
        }
    }
}

impl TimerHandle {
    pub(crate) fn inactive() -> Self {
        Self {
            id: 0,
            queue: Weak::new(),
            active: Arc::new(AtomicBool::new(false)),
            cancelled: true,
            detach_on_drop: false,
        }
    }

    /// Keep the timer until explicit [`Self::cancel`] or window/session teardown.
    ///
    /// Drop no longer cancels — use for `on_start` interval timers so callers need not
    /// stash handles in `Arc<Mutex<Vec<_>>>`（[#180](docs/决策.md#d180)）。
    pub fn detach(mut self) {
        self.detach_on_drop = true;
    }

    pub fn cancel(mut self) {
        self.detach_on_drop = false;
        self.cancel_inner();
    }

    fn cancel_inner(&mut self) {
        if self.cancelled {
            return;
        }
        self.cancelled = true;
        self.active.store(false, Ordering::Release);
        if let Some(queue) = self.queue.upgrade() {
            queue
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .entries
                .remove(&self.id);
        }
    }
}

impl Drop for TimerHandle {
    fn drop(&mut self) {
        if !self.detach_on_drop {
            self.cancel_inner();
        }
    }
}

#[cfg(test)]
#[path = "../tests/app/app_timer.rs"]
mod tests;
