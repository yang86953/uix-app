use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::ui::view::ViewNode;

// 单个窗口轮次最多执行固定数量的主线程任务，避免自投递队列独占 owner thread。
const MAX_JOBS_PER_DRAIN: usize = 64;

pub(crate) struct MainThreadContext<'a> {
    pending_root: &'a mut Option<ViewNode>,
    reconcile_pending: &'a mut bool,
}

impl<'a> MainThreadContext<'a> {
    pub(crate) fn new(
        pending_root: &'a mut Option<ViewNode>,
        reconcile_pending: &'a mut bool,
    ) -> Self {
        Self {
            pending_root,
            reconcile_pending,
        }
    }

    pub(crate) fn update_root(&mut self, root: ViewNode) {
        *self.pending_root = Some(root);
        *self.reconcile_pending = true;
    }
}

type MainThreadJob = Box<dyn for<'a> FnOnce(&mut MainThreadContext<'a>) + Send>;

/// drain 的锁外任务批次；异常展开时把未执行任务按原顺序放回队首。
struct MainThreadDrainBatch<'a> {
    pending: &'a Mutex<VecDeque<MainThreadJob>>,
    clear_epoch: &'a AtomicU64,
    batch_epoch: u64,
    jobs: VecDeque<MainThreadJob>,
}

impl Drop for MainThreadDrainBatch<'_> {
    fn drop(&mut self) {
        if self.jobs.is_empty() {
            return;
        }
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        // clear 在线性化点之后不得因异常恢复而复活旧批次。
        if self.clear_epoch.load(Ordering::Acquire) != self.batch_epoch {
            return;
        }
        // 未执行的旧任务必须先于执行期间新投递的任务，保持全局 FIFO。
        self.jobs.append(&mut pending);
        std::mem::swap(&mut *pending, &mut self.jobs);
    }
}

#[derive(Clone, Default)]
pub(crate) struct MainThreadQueue {
    pending: Arc<Mutex<VecDeque<MainThreadJob>>>,
    clear_epoch: Arc<AtomicU64>,
}

impl MainThreadQueue {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn enqueue<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.enqueue_with_context(move |_| f());
    }

    pub(crate) fn enqueue_with_context<F>(&self, f: F)
    where
        F: for<'a> FnOnce(&mut MainThreadContext<'a>) + Send + 'static,
    {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(Box::new(f));
    }

    pub(crate) fn drain(&self, context: &mut MainThreadContext<'_>) -> bool {
        let mut ran = false;
        let mut remaining = MAX_JOBS_PER_DRAIN;
        let mut batch = MainThreadDrainBatch {
            pending: self.pending.as_ref(),
            clear_epoch: self.clear_epoch.as_ref(),
            batch_epoch: self.clear_epoch.load(Ordering::Acquire),
            jobs: VecDeque::new(),
        };
        // 每一波任务只锁队列一次；回调执行期仍不持锁，并继续接纳自投递任务。
        while remaining > 0 {
            {
                let mut pending = batch.pending.lock().unwrap_or_else(|e| e.into_inner());
                if pending.is_empty() {
                    // 把较大的空缓冲归还共享队列，供后续生产者复用。
                    if batch.jobs.capacity() > pending.capacity() {
                        std::mem::swap(&mut *pending, &mut batch.jobs);
                    }
                    return ran;
                }
                let take = pending.len().min(remaining);
                batch.batch_epoch = batch.clear_epoch.load(Ordering::Acquire);
                if take == pending.len() {
                    // 常见短队列直接移交整个 VecDeque，零元素搬移、零新分配。
                    std::mem::swap(&mut *pending, &mut batch.jobs);
                } else {
                    batch.jobs.extend(pending.drain(..take));
                }
            }

            while !batch.jobs.is_empty() {
                // clear 可能由上一回调触发；旧批次不得继续执行。
                if batch.clear_epoch.load(Ordering::Acquire) != batch.batch_epoch {
                    batch.jobs.clear();
                    break;
                }
                let Some(job) = batch.jobs.pop_front() else {
                    break;
                };
                ran = true;
                remaining -= 1;
                job(context);
            }
        }
        // 完整耗尽本轮预算时返回已执行工作事实，剩余项仍保留在队列中。
        ran
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    }

    pub(crate) fn clear(&self) {
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        pending.clear();
        // 与同一锁内的清空共同形成线性化点，通知锁外 drain 丢弃旧批次。
        self.clear_epoch.fetch_add(1, Ordering::Release);
    }

    // 测试目标保留主线程队列长度观测入口，供队列行为测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

// 主线程队列的公平预算只在单元测试目标中直接观测。
#[cfg(test)]
// 单元测试与队列实现位于同一 Component 边界，可检查私有预算和剩余工作。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../tests/unit/app/queues/main_thread_queue__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
