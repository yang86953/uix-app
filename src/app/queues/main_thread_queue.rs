use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::core::Result;
use crate::platform::windowing::window::PlatformWindow;
use crate::ui::view::ViewNode;

// 单个窗口轮次最多执行固定数量的主线程任务，避免自投递队列独占 owner thread。
const MAX_JOBS_PER_DRAIN: usize = 64;

use crate::ui::WidgetTree;

pub(crate) struct MainThreadContext<'a> {
    pending_root: &'a mut Option<ViewNode>,
    reconcile_pending: &'a mut bool,
    platform_window: &'a mut dyn PlatformWindow,
    tree: &'a mut WidgetTree,
}

impl<'a> MainThreadContext<'a> {
    pub(crate) fn new(
        pending_root: &'a mut Option<ViewNode>,
        reconcile_pending: &'a mut bool,
        platform_window: &'a mut dyn PlatformWindow,
        tree: &'a mut WidgetTree,
    ) -> Self {
        Self {
            pending_root,
            reconcile_pending,
            platform_window,
            tree,
        }
    }

    /// 在当前 UI turn 内短借窗口 WidgetTree，供应用内自动化命令执行语义动作。
    pub(crate) fn tree_mut(&mut self) -> &mut WidgetTree {
        self.tree
    }

    pub(crate) fn update_root(&mut self, root: ViewNode) {
        *self.pending_root = Some(root);
        *self.reconcile_pending = true;
    }

    /// 向当前逐窗 owner 请求原生激活；不把平台窗口借用暴露给上层。
    pub(crate) fn request_activate(&mut self) -> Result<()> {
        self.platform_window.raise()
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

}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../tests-src/app/queues/main_thread_queue_tests.rs"]
mod main_thread_queue_tests;