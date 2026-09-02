//! Diagnostics System 拥有的私有 recovery Module。
//!
//! 该 Module 拥有精确错误码处理器注册及其 RAII 生命周期；公开的
//! action/outcome/subscription 类型是 System 契约。

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, Weak};

use crate::core::{Errc, Error};

thread_local! {
    static RUNNING_RECOVERY: Cell<bool> = const { Cell::new(false) };
}

/// 已注册恢复处理器返回的决定。
pub enum RecoveryAction {
    /// 该处理器无法恢复此错误；尝试下一个匹配的处理器。
    NotHandled,
    /// 处理器已恢复其所属领域持有的不变量。
    Recovered,
    /// 恢复本身以类型化错误失败。
    Failed(Error),
}

/// 运行全部匹配恢复处理器后的结果。
pub enum RecoveryOutcome {
    /// 至少一个匹配处理器成功恢复错误。
    Recovered,
    /// 没有匹配处理器能够恢复错误。
    Unhandled(Error),
    /// 匹配处理器尝试恢复，但恢复过程失败。
    Failed(Error),
}

impl RecoveryOutcome {
    /// 返回恢复流程是否成功恢复了错误。
    pub fn is_recovered(&self) -> bool {
        matches!(self, Self::Recovered)
    }

    /// 当操作未能恢复时返回未处理或恢复错误。
    pub fn into_error(self) -> Option<Error> {
        match self {
            Self::Recovered => None,
            Self::Unhandled(error) | Self::Failed(error) => Some(error),
        }
    }
}

type RecoveryHandler = dyn Fn(&Error) -> RecoveryAction + Send + Sync + 'static;

struct RecoveryEntry {
    id: u64,
    code: Errc,
    handler: Arc<RecoveryHandler>,
}

struct RecoveryState {
    next_id: u64,
    entries: Vec<RecoveryEntry>,
}

pub(super) struct RecoveryModule {
    state: Mutex<RecoveryState>,
}

impl RecoveryModule {
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new(RecoveryState {
                next_id: 1,
                entries: Vec::new(),
            }),
        }
    }

    pub(super) fn register<F>(self: &Arc<Self>, code: Errc, handler: F) -> RecoverySubscription
    where
        F: Fn(&Error) -> RecoveryAction + Send + Sync + 'static,
    {
        let mut state = self.lock_state();
        let id = state.next_id;
        state.next_id = state.next_id.saturating_add(1);
        state.entries.push(RecoveryEntry {
            id,
            code,
            handler: Arc::new(handler),
        });
        RecoverySubscription {
            id,
            registry: Arc::downgrade(self),
        }
    }

    pub(super) fn attempt(&self, error: Error) -> RecoveryOutcome {
        // 进入恢复尝试即完成诊断处置：错误已接触诊断通道。
        error.mark_observed();
        // 恢复过程内不允许嵌套再次触发恢复，防止递归。
        let Some(_guard) = RecoveryGuard::enter() else {
            return RecoveryOutcome::Unhandled(error);
        };

        // 快照当前匹配该错误码的处理器（含 id），随后在锁外执行。
        let handlers = {
            let state = self.lock_state();
            state
                .entries
                .iter()
                .filter(|entry| entry.code == error.code())
                .map(|entry| (entry.id, Arc::clone(&entry.handler)))
                .collect::<Vec<_>>()
        };

        // 按注册顺序逐一尝试；handler 被 panic 包裹，避免破坏诊断系统。
        for (handler_id, handler) in handlers {
            match catch_unwind(AssertUnwindSafe(|| handler(&error))) {
                Ok(RecoveryAction::NotHandled) => {}
                Ok(RecoveryAction::Recovered) => {
                    tracing::info!(
                        target: "uix::diagnostics",
                        error_code = %error.code(),
                        recovery_handler_id = handler_id,
                        "registered recovery restored the error invariant"
                    );
                    return RecoveryOutcome::Recovered;
                }
                // 恢复失败：保留原始错误作为来源链。
                Ok(RecoveryAction::Failed(recovery_error)) => {
                    let recovery_error = recovery_error.with_appended_source(error);
                    return RecoveryOutcome::Failed(
                        Error::new(recovery_error.code(), "registered error recovery failed")
                            .with_source(recovery_error),
                    );
                }
                // 处理器 panic：转换为类型化错误并保留原始错误。
                Err(_) => {
                    return RecoveryOutcome::Failed(
                        Error::new(Errc::TaskAbandoned, "registered error recovery panicked")
                            .with_source(error),
                    );
                }
            }
        }

        // 所有匹配处理器都未能处理，原样返回未处理结果。
        RecoveryOutcome::Unhandled(error)
    }

    fn remove(&self, id: u64) {
        self.lock_state().entries.retain(|entry| entry.id != id);
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, RecoveryState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// 一个精确 `Errc` 恢复处理器的 RAII 注册。
///
/// 丢弃它会阻止未来的 attempt 选择该处理器。已经克隆了处理器的 attempt
/// 可能在不持有注册表锁的情况下多完成一次。
pub struct RecoverySubscription {
    id: u64,
    registry: Weak<RecoveryModule>,
}

impl Drop for RecoverySubscription {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade() {
            registry.remove(self.id);
        }
    }
}

struct RecoveryGuard;

impl RecoveryGuard {
    fn enter() -> Option<Self> {
        RUNNING_RECOVERY.with(|running| {
            if running.replace(true) {
                None
            } else {
                Some(Self)
            }
        })
    }
}

impl Drop for RecoveryGuard {
    fn drop(&mut self) {
        RUNNING_RECOVERY.with(|running| running.set(false));
    }
}
