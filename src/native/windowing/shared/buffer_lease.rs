//! Atomic ownership for compositor-controlled reusable buffers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub(crate) struct BufferLease {
    busy: Arc<AtomicBool>,
}

impl BufferLease {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn try_acquire(&self) -> bool {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn release(&self) {
        self.busy.store(false, Ordering::Release);
    }
}
