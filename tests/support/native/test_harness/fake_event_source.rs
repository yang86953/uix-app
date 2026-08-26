//! Fake 事件源 — 可注入、可断言的事件队列。
//!
//! 实现 `OsEventSource`，通过 blanket impl 自动获得 `IEventLoop`。
//! 支持注入事件序列、追踪已处理事件、模拟退出信号。

use crate::native::windowing::shared::OsEventSource;
use crate::platform::windowing::event::{EventLoopWaker, UiEvent};
use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct FakeEventSourceState {
    /// 待处理事件队列（FIFO）
    pub events: VecDeque<UiEvent>,
    /// 已处理事件（从队列弹出即记录）
    pub processed: Vec<UiEvent>,
    /// dispatch 方法返回 false（模拟平台退出）
    pub should_exit: bool,
    pub dispatch_pending_calls: usize,
    pub dispatch_blocking_calls: usize,
    pub dispatch_timeout_calls: usize,
    pub dispatch_timeout_durations: Vec<Duration>,
    pub blocking_events: VecDeque<UiEvent>,
    pub timeout_events: VecDeque<UiEvent>,
    pub wake_calls: Arc<AtomicUsize>,
    pub exit_after_pending_calls: Option<usize>,
    pub exit_after_blocking_calls: Option<usize>,
    pub exit_after_timeout_calls: Option<usize>,
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

    pub fn wake_count(&self) -> usize {
        self.state.wake_calls.load(Ordering::Relaxed)
    }

    /// 清除所有事件和记录
    pub fn clear(&mut self) {
        self.state.events.clear();
        self.state.processed.clear();
        self.state.should_exit = false;
        self.state.dispatch_pending_calls = 0;
        self.state.dispatch_blocking_calls = 0;
        self.state.dispatch_timeout_calls = 0;
        self.state.dispatch_timeout_durations.clear();
        self.state.blocking_events.clear();
        self.state.timeout_events.clear();
        self.state.wake_calls.store(0, Ordering::Relaxed);
        self.state.exit_after_pending_calls = None;
        self.state.exit_after_blocking_calls = None;
        self.state.exit_after_timeout_calls = None;
    }
}

impl Default for FakeEventSource {
    fn default() -> Self {
        Self::new()
    }
}

impl OsEventSource for FakeEventSource {
    fn waker(&self) -> EventLoopWaker {
        let wake_calls = self.state.wake_calls.clone();
        EventLoopWaker::new(move || {
            wake_calls.fetch_add(1, Ordering::Relaxed);
        })
    }

    fn dispatch_pending(&mut self) -> bool {
        self.state.dispatch_pending_calls += 1;
        if self
            .state
            .exit_after_pending_calls
            .is_some_and(|limit| self.state.dispatch_pending_calls >= limit)
        {
            self.state.should_exit = true;
        }
        !self.state.should_exit
    }

    fn dispatch_blocking(&mut self) -> bool {
        self.state.dispatch_blocking_calls += 1;
        if let Some(event) = self.state.blocking_events.pop_front() {
            self.state.events.push_back(event);
        }
        if self
            .state
            .exit_after_blocking_calls
            .is_some_and(|limit| self.state.dispatch_blocking_calls >= limit)
        {
            self.state.should_exit = true;
        }
        !self.state.should_exit
    }

    fn dispatch_timeout(&mut self, timeout: Duration) -> bool {
        self.state.dispatch_timeout_calls += 1;
        self.state.dispatch_timeout_durations.push(timeout);
        if let Some(event) = self.state.timeout_events.pop_front() {
            self.state.events.push_back(event);
        }
        if self
            .state
            .exit_after_timeout_calls
            .is_some_and(|limit| self.state.dispatch_timeout_calls >= limit)
        {
            self.state.should_exit = true;
        }
        !self.state.should_exit
    }

    fn next_event(&mut self) -> Option<UiEvent> {
        let ev = self.state.events.pop_front()?;
        self.state.processed.push(ev.clone());
        Some(ev)
    }
}
