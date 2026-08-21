//! 平台无关事件协议 — 事件、载荷、分发总线与事件循环。

pub(crate) mod bus;
pub(crate) mod types;

pub(crate) use bus::*;
pub(crate) use types::*;

use std::sync::Arc;

/// 可跨线程持有的事件循环唤醒句柄。
#[derive(Clone)]
pub struct EventLoopWaker {
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl EventLoopWaker {
    /// 从线程安全的唤醒闭包创建句柄。
    pub fn new<F>(wake: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        Self {
            wake: Arc::new(wake),
        }
    }

    /// 请求事件循环尽快处理新工作。
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
pub(crate) trait IEventLoop {
    fn waker(&self) -> EventLoopWaker;
    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
    fn wait_timeout(
        &mut self,
        timeout: std::time::Duration,
        callback: &dyn Fn(&UiEvent) -> bool,
    ) -> bool;
}
