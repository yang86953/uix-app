//! 事件协议 — 平台事件、载荷、事件总线与事件循环。

use std::sync::Arc;

pub mod bus;
pub mod types;

pub use bus::*;
pub use types::*;

#[derive(Clone)]
pub struct EventLoopWaker {
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl EventLoopWaker {
    pub fn new<F>(wake: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        Self {
            wake: Arc::new(wake),
        }
    }

    pub fn wake(&self) {
        (self.wake)();
    }
}

impl Default for EventLoopWaker {
    fn default() -> Self {
        Self::new(|| {})
    }
}

/// 事件循环 — 从 OS 拉取事件并分发给回调。
pub trait IEventLoop {
    fn waker(&self) -> EventLoopWaker;
    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
    fn wait_timeout(
        &mut self,
        timeout: std::time::Duration,
        callback: &dyn Fn(&UiEvent) -> bool,
    ) -> bool;
}
