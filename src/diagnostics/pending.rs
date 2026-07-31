//! Bounded callback-to-owner failure delivery for one Diagnostics System.
//!
//! Callback code may only enqueue a typed failure. The source owner drains it
//! later at an owner-thread boundary, where recovery or final reporting is
//! allowed. The queue is shared by all sources in one runtime, while each
//! source receives only its own failures.

use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

use crate::core::{Errc, Error};

/// Fixed runtime budget for callback failures. Overflow is observable as one
/// typed `InsufficientResources` failure at the source's owner boundary.
pub(crate) const PENDING_FAILURE_CAPACITY: usize = 64;

#[derive(Clone)]
pub(crate) struct PendingFailureQueue {
    inner: Arc<PendingFailureQueueInner>,
}

struct PendingFailureQueueInner {
    next_source_id: AtomicU64,
    pending: Mutex<VecDeque<PendingFailure>>,
}

struct PendingFailure {
    source_id: u64,
    error: Error,
}

/// One owner identity within a shared runtime queue.
///
/// A source is intentionally cheap to clone: one clone stays with the
/// owner-thread resource and callback clones can enqueue without borrowing the
/// resource. Dropping the final clone closes the source and discards any stale
/// entries, preventing teardown callbacks from reaching a replacement owner.
#[derive(Clone)]
pub(crate) struct PendingFailureSource {
    inner: Arc<PendingFailureSourceInner>,
}

struct PendingFailureSourceInner {
    queue: Arc<PendingFailureQueueInner>,
    id: u64,
    dropped: AtomicU64,
    closed: AtomicBool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PendingFailureEnqueue {
    Queued,
    Overflowed,
    Closed,
}

impl PendingFailureQueue {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(PendingFailureQueueInner {
                next_source_id: AtomicU64::new(1),
                pending: Mutex::new(VecDeque::with_capacity(PENDING_FAILURE_CAPACITY)),
            }),
        }
    }

    pub(crate) fn source(&self) -> PendingFailureSource {
        let id = self.inner.next_source_id.fetch_add(1, Ordering::Relaxed);
        PendingFailureSource {
            inner: Arc::new(PendingFailureSourceInner {
                queue: Arc::clone(&self.inner),
                id,
                dropped: AtomicU64::new(0),
                closed: AtomicBool::new(false),
            }),
        }
    }
}

impl Default for PendingFailureQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingFailureSource {
    /// Enqueues without invoking tracing, recovery handlers, subscribers, or
    /// any other user code.
    pub(crate) fn enqueue(&self, error: Error) -> PendingFailureEnqueue {
        if self.inner.closed.load(Ordering::Acquire) {
            return PendingFailureEnqueue::Closed;
        }

        let mut pending = self
            .inner
            .queue
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.inner.closed.load(Ordering::Acquire) {
            return PendingFailureEnqueue::Closed;
        }
        if pending.len() >= PENDING_FAILURE_CAPACITY {
            self.inner.dropped.fetch_add(1, Ordering::Relaxed);
            return PendingFailureEnqueue::Overflowed;
        }
        pending.push_back(PendingFailure {
            source_id: self.inner.id,
            error,
        });
        PendingFailureEnqueue::Queued
    }

    /// Takes one failure in FIFO order. Overflow is surfaced after all queued
    /// real failures so the first observed cause is never replaced by a
    /// synthetic budget error.
    pub(crate) fn take(&self) -> Option<Error> {
        if self.inner.closed.load(Ordering::Acquire) {
            return None;
        }

        let mut pending = self
            .inner
            .queue
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(index) = pending
            .iter()
            .position(|failure| failure.source_id == self.inner.id)
        {
            let Some(failure) = pending.remove(index) else {
                return None;
            };
            return Some(failure.error);
        }
        drop(pending);

        let dropped = self.inner.dropped.swap(0, Ordering::AcqRel);
        (dropped > 0).then(|| {
            Error::new(
                Errc::InsufficientResources,
                format!(
                    "pending failure queue overflow for source {}; dropped {dropped} failure(s)",
                    self.inner.id
                ),
            )
        })
    }

    pub(crate) fn close(&self) {
        self.inner.close();
    }
}

impl PendingFailureSourceInner {
    fn close(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        self.dropped.store(0, Ordering::Release);
        self.queue
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retain(|failure| failure.source_id != self.id);
    }
}

impl Drop for PendingFailureSourceInner {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::{PendingFailureEnqueue, PendingFailureQueue, PENDING_FAILURE_CAPACITY};
    use crate::core::{Errc, Error};

    #[test]
    fn queue_is_fifo_bounded_and_surfaces_overflow_after_real_failures() {
        let queue = PendingFailureQueue::new();
        let source = queue.source();
        for index in 0..PENDING_FAILURE_CAPACITY {
            assert_eq!(
                source.enqueue(Error::new(Errc::PlatformError, format!("failure-{index}"))),
                PendingFailureEnqueue::Queued
            );
        }
        assert_eq!(
            source.enqueue(Error::new(Errc::PlatformError, "overflow")),
            PendingFailureEnqueue::Overflowed
        );

        for index in 0..PENDING_FAILURE_CAPACITY {
            let Some(error) = source.take() else {
                panic!("queued failure must be retained");
            };
            assert_eq!(error.message(), format!("failure-{index}"));
        }
        let Some(overflow) = source.take() else {
            panic!("overflow must become typed failure");
        };
        assert_eq!(overflow.code(), Errc::InsufficientResources);
        assert!(overflow.message().contains("dropped 1 failure"));
        assert!(source.take().is_none());
    }

    #[test]
    fn sources_are_isolated_and_drop_closes_stale_callbacks() {
        let queue = PendingFailureQueue::new();
        let first = queue.source();
        let second = queue.source();
        first.enqueue(Error::new(Errc::IoError, "first"));
        second.enqueue(Error::new(Errc::Timeout, "second"));

        let Some(first_error) = first.take() else {
            panic!("first source");
        };
        let Some(second_error) = second.take() else {
            panic!("second source");
        };
        assert_eq!(first_error.code(), Errc::IoError);
        assert_eq!(second_error.code(), Errc::Timeout);

        let callback = first.clone();
        drop(first);
        callback.enqueue(Error::new(Errc::PlatformError, "late"));
        drop(callback);
        assert!(second.take().is_none());
    }
}
