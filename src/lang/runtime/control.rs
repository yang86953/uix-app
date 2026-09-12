//! Module 与组件 owner 共用的预算与显式取消。

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

/// 每次公开调用重新建立的预算。
#[derive(Debug, Clone)]
pub struct Limits {
    pub steps: u64,
    pub depth: usize,
    pub value_bytes: usize,
    pub value_items: usize,
    pub timeout: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            steps: 100_000,
            depth: 64,
            value_bytes: 1_048_576,
            value_items: 65_536,
            timeout: Duration::from_secs(2),
        }
    }
}

/// 调用方可跨线程请求取消；请求不冒充外部效果回滚。
#[derive(Debug, Clone, Default)]
pub struct Cancellation {
    flag: Arc<AtomicBool>,
    parent: Option<Arc<AtomicBool>>,
    wake: Arc<Mutex<Option<std::thread::Thread>>>,
}
impl Cancellation {
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
        self.wake();
    }
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
            || self
                .parent
                .as_ref()
                .is_some_and(|p| p.load(Ordering::Acquire))
    }
    pub(crate) fn child(&self) -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            parent: Some(self.flag.clone()),
            wake: self.wake.clone(),
        }
    }
    pub(crate) fn attach_worker(&self) {
        *self.wake.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::thread::current());
    }
    pub(crate) fn wake(&self) {
        if let Some(thread) = self.wake.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            thread.unpark();
        }
    }
}
