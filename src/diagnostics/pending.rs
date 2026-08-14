//! 一个 Diagnostics System 的有界回调→owner 失败投递。
//!
//! 回调代码只能入队一个类型化失败。源 owner 稍后在 owner 线程边界排空它，
//! 该处才允许执行恢复或最终上报。队列被同一运行时内的所有源共享，而每个
//! 源只收到属于自己的失败。

use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use crate::core::{Errc, Error};

/// 回调失败的固定运行时预算。溢出会在源的 owner 边界以一条类型化的
/// `InsufficientResources` 失败形式被观察到。
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

/// 共享运行时队列中的一个 owner 身份。
///
/// 源故意设计为廉价克隆：一个克隆留在 owner 线程资源上，回调克隆可以入队
/// 而无需借用该资源。丢弃最后一个克隆会关闭源并丢弃任何过期条目，防止
/// teardown 回调到达一个替代 owner。
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
    /// 入队时不调用 tracing、恢复处理器、订阅者或任何其他用户代码。
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

    /// 按 FIFO 顺序取出一条失败。溢出信号在所有已排队的真实失败之后
    /// 才浮出，保证最先观察到的成因不会被合成预算错误替代。
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
            // 索引来自同一锁保护下的当前队列，缺失时直接返回无结果。
            let failure = pending.remove(index)?;
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
    use super::{PENDING_FAILURE_CAPACITY, PendingFailureEnqueue, PendingFailureQueue};
    use crate::core::{Errc, Error};
    use std::sync::{Arc, Barrier};
    use std::thread;

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

    #[test]
    fn concurrent_callback_producers_preserve_capacity_and_one_overflow_signal() {
        const PRODUCER_COUNT: usize = 4;
        const ATTEMPTS_PER_PRODUCER: usize = PENDING_FAILURE_CAPACITY / 2;

        assert_eq!(PENDING_FAILURE_CAPACITY, 64);
        let queue = PendingFailureQueue::new();
        let source = queue.source();
        let start = Arc::new(Barrier::new(PRODUCER_COUNT));
        let mut producers = Vec::with_capacity(PRODUCER_COUNT);

        for producer_id in 0..PRODUCER_COUNT {
            let source = source.clone();
            let start = Arc::clone(&start);
            producers.push(thread::spawn(move || {
                start.wait();
                let mut queued = 0;
                let mut overflowed = 0;
                for attempt in 0..ATTEMPTS_PER_PRODUCER {
                    let result = source.enqueue(Error::new(
                        Errc::PlatformError,
                        format!("producer-{producer_id}-{attempt}"),
                    ));
                    match result {
                        PendingFailureEnqueue::Queued => queued += 1,
                        PendingFailureEnqueue::Overflowed => overflowed += 1,
                        PendingFailureEnqueue::Closed => {
                            panic!("the active source must not close during producer pressure")
                        }
                    }
                }
                (queued, overflowed)
            }));
        }

        let (queued, overflowed) = producers
            .into_iter()
            .map(|producer| {
                producer
                    .join()
                    .unwrap_or_else(|_| panic!("callback producer must not panic"))
            })
            .fold(
                (0, 0),
                |(queued, overflowed), (producer_queued, producer_overflowed)| {
                    (queued + producer_queued, overflowed + producer_overflowed)
                },
            );
        assert_eq!(queued, PENDING_FAILURE_CAPACITY);
        assert_eq!(overflowed, PRODUCER_COUNT * ATTEMPTS_PER_PRODUCER - queued);

        // 所有生产者只入队。只有 owner 线程排空源，并且必须先看到真实失败，
        // 随后才是一条预算信号。
        let mut real_failures = 0;
        let mut overflow_signals = 0;
        while let Some(error) = source.take() {
            if error.code() == Errc::InsufficientResources {
                overflow_signals += 1;
            } else {
                assert_eq!(error.code(), Errc::PlatformError);
                real_failures += 1;
            }
        }
        assert_eq!(real_failures, PENDING_FAILURE_CAPACITY);
        assert_eq!(overflow_signals, 1);
        assert!(source.take().is_none());
    }

    #[test]
    fn closed_generation_releases_capacity_before_replacement_pressure() {
        let queue = PendingFailureQueue::new();
        let old_generation = queue.source();
        let late_callback = old_generation.clone();
        old_generation.enqueue(Error::new(Errc::PlatformError, "old generation"));
        old_generation.close();

        let replacement = queue.source();
        assert_eq!(
            late_callback.enqueue(Error::new(Errc::PlatformError, "late old generation")),
            PendingFailureEnqueue::Closed
        );
        for index in 0..PENDING_FAILURE_CAPACITY {
            assert_eq!(
                replacement.enqueue(Error::new(
                    Errc::PlatformError,
                    format!("replacement-{index}"),
                )),
                PendingFailureEnqueue::Queued
            );
        }

        let mut replacement_failures = 0;
        while replacement.take().is_some() {
            replacement_failures += 1;
        }
        assert_eq!(replacement_failures, PENDING_FAILURE_CAPACITY);
        assert!(old_generation.take().is_none());
    }
}
