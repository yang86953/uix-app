use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::ui::view::ViewNode;

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
        loop {
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

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

#[cfg(test)]
#[path = "../tests/app/main_thread_queue.rs"]
mod tests;
