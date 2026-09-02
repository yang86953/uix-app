//! Report 即时通知 Component owned by the reporting Module.
//!
//! 每份报告入库后、结构化事件发射完，立即在 report 调用线程同步通知全部
//! 订阅者——宿主不依赖 tracing subscriber 也能「错误一发生就知道」。
//! 通知不递归：处理器内部再产生的报告照常入库与发射事件，但不再进入
//! 通知分发；处理器 panic 被隔离为 emergency 限流输出，不破坏诊断系统。

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, Weak};

use super::super::report::ErrorReport;

thread_local! {
    static NOTIFYING_REPORT: Cell<bool> = const { Cell::new(false) };
}

struct NotificationGuard;

impl NotificationGuard {
    fn enter() -> Option<Self> {
        NOTIFYING_REPORT.with(|notifying| {
            if notifying.replace(true) {
                None
            } else {
                Some(Self)
            }
        })
    }
}

impl Drop for NotificationGuard {
    fn drop(&mut self) {
        NOTIFYING_REPORT.with(|notifying| notifying.set(false));
    }
}

type ReportHandler = dyn Fn(&ErrorReport) + Send + Sync + 'static;

struct NotifierEntry {
    id: u64,
    handler: Arc<ReportHandler>,
}

struct NotifierState {
    next_id: u64,
    entries: Vec<NotifierEntry>,
}

pub(super) struct ReportNotifier {
    state: Mutex<NotifierState>,
}

impl ReportNotifier {
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new(NotifierState {
                next_id: 1,
                entries: Vec::new(),
            }),
        }
    }

    pub(super) fn subscribe<F>(self: &Arc<Self>, handler: F) -> ReportSubscription
    where
        F: Fn(&ErrorReport) + Send + Sync + 'static,
    {
        let mut state = self.lock_state();
        let id = state.next_id;
        state.next_id = state.next_id.saturating_add(1);
        state.entries.push(NotifierEntry {
            id,
            handler: Arc::new(handler),
        });
        ReportSubscription {
            id,
            notifier: Arc::downgrade(self),
        }
    }

    /// 在 report 调用线程同步分发一份刚入库的报告。
    pub(super) fn notify(&self, report: &ErrorReport, emergency_count: &AtomicU64) {
        // 处理器内部再 report 时不重复分发，防止递归通知。
        let Some(_guard) = NotificationGuard::enter() else {
            return;
        };
        // 快照当前处理器，随后在锁外执行；处理器不得重入注册表。
        let handlers = {
            let state = self.lock_state();
            state
                .entries
                .iter()
                .map(|entry| (entry.id, Arc::clone(&entry.handler)))
                .collect::<Vec<_>>()
        };
        for (handler_id, handler) in handlers {
            if catch_unwind(AssertUnwindSafe(|| handler(report))).is_err() {
                super::emit::emergency_notice(
                    emergency_count,
                    &format!("report subscriber {handler_id} panicked"),
                );
            }
        }
    }

    fn remove(&self, id: u64) {
        self.lock_state().entries.retain(|entry| entry.id != id);
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, NotifierState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// 一份报告即时通知的 RAII 订阅。
///
/// 每当 [`crate::diagnostics::Diagnostics`] 产生新报告（应用 `report` 或框架
/// 内部上报），处理器在 report 调用线程同步收到入库后的 [`ErrorReport`]。
/// 丢弃本句柄即停止通知；处理器内部的嵌套上报照常入库但不再次分发。
pub struct ReportSubscription {
    id: u64,
    notifier: Weak<ReportNotifier>,
}

impl Drop for ReportSubscription {
    fn drop(&mut self) {
        if let Some(notifier) = self.notifier.upgrade() {
            notifier.remove(self.id);
        }
    }
}
