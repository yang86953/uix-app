use std::collections::VecDeque;
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

#[derive(Clone, Default)]
pub(crate) struct MainThreadQueue {
    pending: Arc<Mutex<VecDeque<MainThreadJob>>>,
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
        // 只消费本轮预算，预算后进入队列的剩余任务由下一窗口轮次继续处理。
        for _ in 0..MAX_JOBS_PER_DRAIN {
            let job = self
                .pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pop_front();
            let Some(job) = job else {
                return ran;
            };
            ran = true;
            job(context);
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
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
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
