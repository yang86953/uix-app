//! Fake 事件源 — 可注入、可断言的事件队列。
//!
//! 实现 `OsEventSource`，通过 blanket impl 自动获得 `IEventLoop`。
//! 支持注入事件序列、追踪已处理事件、模拟退出信号。

use crate::native::traits::event::UiEvent;
use crate::native::shared::OsEventSource;
use std::collections::VecDeque;

#[derive(Debug, Clone, Default)]
pub struct FakeEventSourceState {
    /// 待处理事件队列（FIFO）
    pub events: VecDeque<UiEvent>,
    /// 已处理事件（从队列弹出即记录）
    pub processed: Vec<UiEvent>,
    /// dispatch 方法返回 false（模拟平台退出）
    pub should_exit: bool,
}

#[derive(Debug)]
pub struct FakeEventSource {
    pub state: FakeEventSourceState,
}

impl FakeEventSource {
    pub fn new() -> Self {
        Self {
            state: FakeEventSourceState::default(),
        }
    }

    /// 注入一个事件到队列末尾
    pub fn inject(&mut self, event: UiEvent) {
        self.state.events.push_back(event);
    }

    /// 批量注入事件
    pub fn inject_all(&mut self, events: impl IntoIterator<Item = UiEvent>) {
        for ev in events {
            self.state.events.push_back(ev);
        }
    }

    /// 队列中剩余事件数
    pub fn pending_count(&self) -> usize {
        self.state.events.len()
    }

    /// 已处理事件数
    pub fn processed_count(&self) -> usize {
        self.state.processed.len()
    }

    /// 清除所有事件和记录
    pub fn clear(&mut self) {
        self.state.events.clear();
        self.state.processed.clear();
        self.state.should_exit = false;
    }
}

impl Default for FakeEventSource {
    fn default() -> Self {
        Self::new()
    }
}

impl OsEventSource for FakeEventSource {
    fn dispatch_pending(&mut self) -> bool {
        !self.state.should_exit
    }

    fn dispatch_blocking(&mut self) -> bool {
        !self.state.should_exit
    }

    fn dispatch_timeout(&mut self, _timeout: std::time::Duration) -> bool {
        !self.state.should_exit
    }

    fn next_event(&mut self) -> Option<UiEvent> {
        let ev = self.state.events.pop_front()?;
        self.state.processed.push(ev.clone());
        Some(ev)
    }
}
