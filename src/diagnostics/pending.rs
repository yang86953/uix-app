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
        // 入队即完成诊断处置：错误已进入投递通道，由 owner 边界消费。
        error.mark_observed();
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
