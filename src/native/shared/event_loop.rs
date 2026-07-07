// ============================================================================
// platform/shared/event_loop.rs — 事件循环共享实现
//
// OsEventSource trait：平台只需实现 3 个必须方法 + 1 个可选方法，
// 自动获得 IEventLoop（poll_event / wait_event / wait_timeout）。
//
// 公开接口 IEventLoop 已迁移至 crate::native::traits 功能模块。
// ============================================================================

use crate::native::traits::event::EventLoopWaker;
use crate::native::traits::event::IEventLoop;
use crate::native::traits::event::UiEvent;
use std::time::Duration;

// ════════════════════════════════════════════════════════════════════════════
// OsEventSource — 平台事件源
// ════════════════════════════════════════════════════════════════════════════

/// 平台事件源：平台特有的事件分发操作。
///
/// ## 必须实现（3 个）
/// - `dispatch_pending` — 非阻塞分发 OS 事件到内部队列
/// - `dispatch_blocking` — 阻塞等待 OS 事件
/// - `next_event` — 从队列弹出一个 UI 事件，无事件返回 None
///
/// ## 可选覆盖（2 个）
/// - `dispatch_timeout` — 默认用 poll + sleep 实现，平台可覆盖为更好的实现
/// - `waker` — 默认 no-op；平台可返回可跨线程唤醒 OS 事件循环的句柄
pub trait OsEventSource {
    /// 非阻塞分发挂起的 OS 事件。返回 `false` 表示退出。
    fn dispatch_pending(&mut self) -> bool;

    /// 阻塞等待 OS 事件。返回 `false` 表示退出。
    fn dispatch_blocking(&mut self) -> bool;

    /// 带超时等待 OS 事件。
    /// 默认实现：poll + sleep。平台应覆盖为原生超时机制（如 `MsgWaitForMultipleObjects`）。
    fn dispatch_timeout(&mut self, timeout: Duration) -> bool {
        if !self.dispatch_pending() {
            return false;
        }
        std::thread::sleep(timeout);
        true
    }

    /// 从事件队列弹出下一个 UI 事件。无事件返回 `None`。
    fn next_event(&mut self) -> Option<UiEvent>;

    /// 返回可从其他线程唤醒事件循环的句柄。
    fn waker(&self) -> EventLoopWaker {
        EventLoopWaker::default()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// blanket impl：所有 OsEventSource 自动获得 IEventLoop
// ════════════════════════════════════════════════════════════════════════════

impl<T: OsEventSource> IEventLoop for T {
    fn waker(&self) -> EventLoopWaker {
        OsEventSource::waker(self)
    }

    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        if !self.dispatch_pending() {
            return false;
        }
        while let Some(event) = self.next_event() {
            if !callback(&event) {
                return false;
            }
        }
        true
    }

    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        if !self.poll_event(callback) {
            return false;
        }
        if !self.dispatch_blocking() {
            return false;
        }
        while let Some(event) = self.next_event() {
            if !callback(&event) {
                return false;
            }
        }
        true
    }

    fn wait_timeout(&mut self, timeout: Duration, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        if !self.dispatch_timeout(timeout) {
            return false;
        }
        while let Some(event) = self.next_event() {
            if !callback(&event) {
                return false;
            }
        }
        true
    }
}
