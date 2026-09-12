use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use crate::app::queues::active_work_registry::TimerId;
use crate::app::queues::clock::{AppClock, system_clock};

#[derive(Clone)]
pub(crate) struct AppTimerQueue {
    inner: Arc<Mutex<AppTimerQueueInner>>,
    clock: Arc<dyn AppClock>,
}

struct AppTimerQueueInner {
    next_id: TimerId,
    cancellation_epoch: u64,
    deadline_revision: u64,
    entries: BTreeMap<TimerId, AppTimerEntry>,
    removal_waker: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Default for AppTimerQueueInner {
    fn default() -> Self {
        Self {
            next_id: 1,
            cancellation_epoch: 0,
            deadline_revision: 0,
            entries: BTreeMap::new(),
            removal_waker: None,
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

/// 控制定时器取消与脱离所有权语义的句柄。
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

    /// Associate this per-window queue with its owning runtime wake source.
    ///
    /// Handles read the current callback when they actually remove a pending
    /// timer, so handles created before runtime startup still use the installed
    /// native waker once the event loop begins.
    pub(crate) fn set_removal_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .removal_waker = Some(waker);
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

    pub(crate) fn deadlines_into_if_changed(
        &self,
        known_revision: Option<u64>,
        deadlines: &mut Vec<(TimerId, Instant)>,
    ) -> Option<u64> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if known_revision == Some(inner.deadline_revision) {
            return None;
        }
        deadlines.clear();
        deadlines.extend(
            inner
                .entries
                .iter()
                .map(|(&id, entry)| (id, entry.deadline)),
        );
        Some(inner.deadline_revision)
    }

    pub(crate) fn cancel_all(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.cancellation_epoch = inner.cancellation_epoch.wrapping_add(1);
        for entry in inner.entries.values() {
            entry.active.store(false, Ordering::Release);
        }
        if !inner.entries.is_empty() {
            inner.entries.clear();
            inner.deadline_revision = inner.deadline_revision.wrapping_add(1);
        }
    }

    pub(crate) fn fire(&self, id: TimerId, now: Instant) -> bool {
        let mut entry = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let entry = inner.entries.remove(&id);
            if entry.is_some() {
                inner.deadline_revision = inner.deadline_revision.wrapping_add(1);
            }
            entry
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
                inner.deadline_revision = inner.deadline_revision.wrapping_add(1);
            }
        }

        true
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
        inner.deadline_revision = inner.deadline_revision.wrapping_add(1);
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
    /// stash handles in `Arc<Mutex<Vec<_>>>`（公开用法见仓库 `docs/使用/定时与异步.md`）。
    pub fn detach(mut self) {
        self.detach_on_drop = true;
    }

    /// 立即取消定时器，并唤醒所属运行时重新计算截止时间。
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
        let removal_waker = self.queue.upgrade().and_then(|queue| {
            let mut inner = queue.lock().unwrap_or_else(|e| e.into_inner());
            if inner.entries.remove(&self.id).is_some() {
                inner.deadline_revision = inner.deadline_revision.wrapping_add(1);
                inner.removal_waker.clone()
            } else {
                None
            }
        });
        // Never invoke platform/runtime code while holding the timer queue.
        if let Some(wake) = removal_waker {
            wake();
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

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../tests-src/app/queues/app_timer_tests.rs"]
mod app_timer_tests;