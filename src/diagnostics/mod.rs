//! Runtime-scoped error observation shared by UIX and its applications.
//!
//! SMC boundary for this vertical slice:
//! - `Diagnostics` is the public platform System and owns runtime state,
//!   cross-module orchestration, and failure/reporting policy.
//! - `reporting` and `recovery` are private Modules owned by one System
//!   instance; neither Module is exported or discovers the other.
//! - report draft construction, bounded storage, and tracing emission are
//!   narrow Components selected and owned by the reporting Module.
//!
//! # 私有 Module 边界 deny 证据（compile-fail）
//!
//! `reporting` / `recovery` 是 Diagnostics System 的私有 Module，`pending` /
//! `report` / `config` 是 System 私有实现文件；它们对外一律不可达。以下契约
//! 锁定该边界，任何把私有 Module 提升为公开模块的改动都会编译失败。
//!
//! ```compile_fail
//! use uix::diagnostics::reporting::ReportingModule;
//! ```
//!
//! ```compile_fail
//! use uix::diagnostics::recovery::RecoveryModule;
//! ```
//!
//! ```compile_fail
//! use uix::diagnostics::pending::PendingFailureQueue;
//! ```
//!
//! ```compile_fail
//! use uix::diagnostics::report::ReportOrigin;
//! ```
//!
//! ```compile_fail
//! use uix::diagnostics::config::DiagnosticsConfig;
//! ```
//!
//! ```compile_fail
//! use uix::diagnostics::crash::CrashReport;
//! ```

mod config;
mod crash;
mod pending;
mod recovery;
mod report;
mod reporting;

use std::panic::Location;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::core::Error;

pub use config::{BacktracePolicy, DiagnosticsConfig};
pub(crate) use pending::{PendingFailureQueue, PendingFailureSource};
pub use recovery::{RecoveryAction, RecoveryOutcome, RecoverySubscription};

/// 测试专用：保护进程级全局 panic hook 的安装/恢复窗口。
///
/// panic hook 是进程全局状态。`crash` 的 hook 测试与故意触发 panic 的 ABI
/// 测试（`wnd_proc` / TSF thunk）并行时，后者的 panic 会被前者的全局 hook
/// 捕获并写入其崩溃目录，导致目录断言失败。这些测试共享本锁串行执行。
#[cfg(test)]
pub(crate) static PANIC_HOOK_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub(crate) use report::ReportOrigin;
pub use report::{DiagnosticsSnapshot, ErrorReport, ReportId};
use reporting::ReportingModule;

static NEXT_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);

struct DiagnosticsInner {
    runtime_id: u64,
    config: DiagnosticsConfig,
    pending_failures: PendingFailureQueue,
    reporting: ReportingModule,
    recovery: Arc<recovery::RecoveryModule>,
}

/// Public Diagnostics System handle for one runtime.
///
/// The handle exposes only the System contract. Its reporting and recovery
/// Modules, their Components, and their mutable state remain private to this
/// runtime instance.
#[derive(Clone)]
pub struct Diagnostics {
    inner: Arc<DiagnosticsInner>,
}

impl Diagnostics {
    pub fn new(config: DiagnosticsConfig) -> Self {
        let runtime_id = NEXT_RUNTIME_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: Arc::new(DiagnosticsInner {
                runtime_id,
                reporting: ReportingModule::new(&config),
                config,
                pending_failures: PendingFailureQueue::new(),
                recovery: Arc::new(recovery::RecoveryModule::new()),
            }),
        }
    }

    /// Reports an application-owned typed error at its final responsibility
    /// boundary.
    ///
    /// > **tracing 初始化顺序**:事件经 `tracing::event!` 发射,tracing 对每个
    /// > callsite 的 interest 首次注册即缓存。若在本进程设置 tracing
    /// > subscriber 之前调用过本方法,且此后事件不可见,调用
    /// > [`Self::rebuild_tracing_interest_cache`] 即可恢复。
    #[track_caller]
    pub fn report(&self, error: Error) -> ReportId {
        self.report_with_origin(error, ReportOrigin::application())
    }

    /// Re-evaluates tracing's process-wide callsite interest cache.
    ///
    /// tracing 对每个 callsite 的 `Interest` 首次注册即缓存;若某个 callsite 在
    /// 尚无任何 subscriber 的上下文下注册,会被缓存为 `never`,之后即使设置了
    /// subscriber 也收不到该 callsite 的事件(见 `diagnostics::reporting::emit`)。
    /// 应用在初始化 tracing subscriber 后调用本方法即可刷新全部 callsite 的
    /// interest,恢复诊断事件可见性。
    pub fn rebuild_tracing_interest_cache(&self) {
        tracing::callsite::rebuild_interest_cache();
    }

    /// Returns an immutable point-in-time snapshot ordered by `ReportId`.
    pub fn snapshot(&self) -> DiagnosticsSnapshot {
        self.inner.reporting.snapshot()
    }

    /// Returns the runtime queue used by native callbacks to deliver typed
    /// failures to their owner-thread resource. The queue never invokes user
    /// code while a callback is running.
    pub(crate) fn pending_failure_queue(&self) -> PendingFailureQueue {
        self.inner.pending_failures.clone()
    }

    /// Registers recovery logic for one exact typed error code.
    ///
    /// Handlers run synchronously, in stable registration order, only when
    /// [`Self::attempt_recovery`] is called from a safe owner/caller thread.
    /// Native callbacks must enqueue failures instead of invoking user code.
    pub fn on_error<F>(&self, code: crate::core::Errc, handler: F) -> RecoverySubscription
    where
        F: Fn(&Error) -> RecoveryAction + Send + Sync + 'static,
    {
        self.inner.recovery.register(code, handler)
    }

    /// Attempts registered recovery without implicitly reporting an
    /// unhandled error.
    ///
    /// Callers may keep propagating the typed error and report it only if they
    /// become its final responsibility boundary.
    pub fn attempt_recovery(&self, error: Error) -> RecoveryOutcome {
        self.inner.recovery.attempt(error)
    }

    /// Framework-only reporting entry with a bounded, typed origin.
    #[track_caller]
    pub(crate) fn report_with_origin(&self, error: Error, origin: ReportOrigin) -> ReportId {
        let report_site = Location::caller();
        self.inner.reporting.report(
            self.inner.runtime_id,
            self.inner.config.backtrace,
            error,
            origin,
            report_site,
        )
    }

    pub(crate) fn crash_report_directory(&self) -> Option<&std::path::Path> {
        self.inner.config.crash_report_directory.as_deref()
    }

    pub(crate) fn runtime_id(&self) -> u64 {
        self.inner.runtime_id
    }

    /// Installs the framework panic hook bound to this runtime.
    ///
    /// Panics are never swallowed: the previous hook is always invoked, so
    /// default output and unwind/abort semantics are preserved. When a crash
    /// directory is configured, a bounded crash report is written atomically
    /// before forwarding.
    pub(crate) fn install_panic_hook(&self) {
        crash::install_panic_hook(self.clone());
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new(DiagnosticsConfig::default())
    }
}
