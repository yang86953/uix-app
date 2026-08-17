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
mod tests {
    // 复用当前模块的队列、上下文与预算常量。
    use super::*;

    // 验证回调继续向同一队列投递时，单次 drain 仍会在固定预算处返回。
    #[test]
    // 执行主线程队列自投递公平性场景。
    fn self_posted_work_is_left_for_the_next_drain() {
        // 创建目标窗口独占的主线程队列。
        let queue = MainThreadQueue::new();
        // 克隆同一队列，供首个回调继续投递工作。
        let producer = queue.clone();
        // 首个回调在执行时再投递一个完整预算的任务。
        queue.enqueue(move || {
            // 生成足以跨越当前轮次预算的后续任务。
            for _ in 0..MAX_JOBS_PER_DRAIN {
                // 每个后续任务都为空操作，只观测队列调度语义。
                producer.enqueue(|| {});
            }
        });
        // 测试上下文不提交新根。
        let mut pending_root = None;
        // 测试上下文初始没有协调请求。
        let mut reconcile_pending = false;
        // 把窗口拥有的根与协调状态借给本轮任务。
        let mut context = MainThreadContext::new(&mut pending_root, &mut reconcile_pending);

        // 第一轮必须执行工作并在预算耗尽后返回。
        assert!(queue.drain(&mut context));
        // 首个回调加后续任务共超出预算一项，该项必须仍在队列中。
        assert_eq!(queue.len(), 1);
        // 下一轮继续执行保留的最后一项。
        assert!(queue.drain(&mut context));
        // 两轮结束后队列应完整清空且没有任务丢失。
        assert_eq!(queue.len(), 0);
    }
}
