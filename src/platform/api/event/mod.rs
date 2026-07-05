//! 事件协议 — 平台事件、载荷、事件总线与事件循环。

pub mod bus;
pub mod types;

pub use bus::*;
pub use types::*;

/// 事件循环 — 从 OS 拉取事件并分发给回调。
pub trait IEventLoop {
    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
    fn wait_timeout(
        &mut self,
        timeout: std::time::Duration,
        callback: &dyn Fn(&UiEvent) -> bool,
    ) -> bool;
}
