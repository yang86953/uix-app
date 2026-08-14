//! 事件协议 — 平台事件、载荷、事件总线与事件循环。

pub(crate) mod bus;
pub(crate) mod types;

pub(crate) use bus::*;
pub(crate) use types::*;

// 事件循环唤醒器已收口到 platform 公开面；此处保持既有
// `crate::native::windowing::event::EventLoopWaker` 路径可解析。
pub(crate) use crate::platform::windowing::EventLoopWaker;

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
