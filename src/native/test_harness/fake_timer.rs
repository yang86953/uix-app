//! Fake 定时器 — 手动推进时间，不依赖真实时钟。
//!
//! 主动推进时间轴，触发到期的定时器回调。
//! 支持单次和重复定时器，追踪所有 set/clear 操作。

use crate::native::capabilities::system::ITimer;
use crate::native::Result;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct FakeTimerEntry {
    pub id: u32,
    pub interval_ms: u32,
    pub repeating: bool,
    pub remaining: Duration,
}

#[derive(Debug, Clone)]
pub struct FakeTimerState {
    /// 活跃的定时器
    pub timers: Vec<FakeTimerEntry>,
    /// `set` 调用记录 (interval_ms, repeating)
    pub set_calls: Vec<(u32, bool)>,
    /// `clear` 调用记录 (timer_id)
    pub clear_calls: Vec<u32>,
    pub next_id: u32,
    pub current_time: Duration,
}

impl Default for FakeTimerState {
    fn default() -> Self {
        Self {
            timers: Vec::new(),
            set_calls: Vec::new(),
            clear_calls: Vec::new(),
            next_id: 1,
            current_time: Duration::ZERO,
        }
    }
}

#[derive(Debug)]
pub struct FakeTimer {
    pub state: FakeTimerState,
}

impl FakeTimer {
    pub fn new() -> Self {
        Self {
            state: FakeTimerState::default(),
        }
    }

    /// 推进时间，触发到期的定时器
    ///
    /// 返回本次触发（到期）的定时器 ID 列表。
    pub fn advance(&mut self, delta: Duration) -> Vec<u32> {
        self.state.current_time += delta;
        let mut fired = Vec::new();

        // 更新所有定时器的剩余时间
        for entry in &mut self.state.timers {
            if entry.remaining > delta {
                entry.remaining -= delta;
            } else {
                entry.remaining = Duration::ZERO;
            }
        }

        // 收集到期的定时器
        let mut i = 0;
        while i < self.state.timers.len() {
            if self.state.timers[i].remaining == Duration::ZERO {
                let id = self.state.timers[i].id;
                fired.push(id);

                if self.state.timers[i].repeating {
                    // 重复定时器：重置剩余时间
                    self.state.timers[i].remaining =
                        Duration::from_millis(self.state.timers[i].interval_ms as u64);
                    i += 1;
                } else {
                    // 单次定时器：移除
                    self.state.timers.remove(i);
                }
            } else {
                i += 1;
            }
        }

        fired
    }

    /// 推进到所有定时器到期（用于快速测试）
    pub fn advance_to_idle(&mut self) -> Vec<u32> {
        let max = self
            .state
            .timers
            .iter()
            .map(|t| t.interval_ms)
            .max()
            .unwrap_or(0);
        self.advance(Duration::from_millis(max as u64 + 1))
    }

    /// 挂起的定时器数量
    pub fn pending_count(&self) -> usize {
        self.state.timers.len()
    }

    /// 是否有指定 ID 的定时器仍在挂起
    pub fn is_pending(&self, id: u32) -> bool {
        self.state.timers.iter().any(|t| t.id == id)
    }

    pub fn clear_history(&mut self) {
        self.state.set_calls.clear();
        self.state.clear_calls.clear();
    }
}

impl Default for FakeTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl ITimer for FakeTimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> Result<u32> {
        let id = self.state.next_id;
        self.state.next_id += 1;
        self.state.timers.push(FakeTimerEntry {
            id,
            interval_ms,
            repeating,
            remaining: Duration::from_millis(interval_ms as u64),
        });
        self.state.set_calls.push((interval_ms, repeating));
        Ok(id)
    }

    fn clear(&mut self, id: u32) -> Result<()> {
        self.state.timers.retain(|t| t.id != id);
        self.state.clear_calls.push(id);
        Ok(())
    }
}
