//! 一个 Diagnostics System 的瞬态错误观察去重 Module。
//!
//! 瞬态失败（自愈重试、fallback 保持武装、高频平台噪声）按决策矩阵不
//! 进入报告存储，但逐条刷屏同样污染日志；本 Module 按 `(target, reason)`
//! 冷却窗口去重：首条立即发射，窗口内重复观察只累计抑制计数，冷却到期
//! 后的下一条携带窗口内累计数。键由调用点的静态字符串对组成，数量在
//! 编译期有界，不违反错误风暴的内存上界。

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 同一 `(target, reason)` 观察的冷却窗口。
const TRANSIENT_COOLDOWN: Duration = Duration::from_secs(30);

pub(super) struct TransientObservationModule {
    observed: Mutex<HashMap<(&'static str, &'static str), TransientState>>,
    // 累计观察次数（含被抑制的重复），供诊断快照回看瞬态量级。
    total: AtomicU64,
    // 累计被冷却窗口抑制的观察次数。
    suppressed: AtomicU64,
}

struct TransientState {
    last_emitted: Instant,
    suppressed: u64,
}

/// 一次观察的去重判定结果，由 System 负责实际事件发射。
pub(super) enum TransientObservation {
    /// 发射本条事件；`suppressed_in_window` 为上个冷却窗口内累计的抑制数。
    Emit { suppressed_in_window: u64 },
    /// 冷却窗口内的重复观察，只累计计数。
    Suppressed,
}

impl TransientObservationModule {
    pub(super) fn new() -> Self {
        Self {
            observed: Mutex::new(HashMap::new()),
            total: AtomicU64::new(0),
            suppressed: AtomicU64::new(0),
        }
    }

    /// 返回累计瞬态观察次数（含抑制）与其中被抑制的次数。
    pub(super) fn counts(&self) -> (u64, u64) {
        (
            self.total.load(Ordering::Relaxed),
            self.suppressed.load(Ordering::Relaxed),
        )
    }

    pub(super) fn observe(&self, target: &'static str, reason: &'static str) -> TransientObservation {
        self.total.fetch_add(1, Ordering::Relaxed);
        let mut observed = self
            .observed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = Instant::now();
        match observed.entry((target, reason)) {
            // 首次观察立即发射，保证瞬态失败从第一条就可见。
            Entry::Vacant(slot) => {
                slot.insert(TransientState {
                    last_emitted: now,
                    suppressed: 0,
                });
                TransientObservation::Emit {
                    suppressed_in_window: 0,
                }
            }
            Entry::Occupied(mut slot) => {
                let state = slot.get_mut();
                if now.duration_since(state.last_emitted) < TRANSIENT_COOLDOWN {
                    state.suppressed = state.suppressed.saturating_add(1);
                    self.suppressed.fetch_add(1, Ordering::Relaxed);
                    return TransientObservation::Suppressed;
                }
                // 冷却到期：携带窗口内累计数发射并重置窗口。
                let suppressed_in_window = state.suppressed;
                state.suppressed = 0;
                state.last_emitted = now;
                TransientObservation::Emit {
                    suppressed_in_window,
                }
            }
        }
    }
}
