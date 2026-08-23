//! UIX 与其应用共享的运行时作用域错误观察。
//!
//! 本纵向切片的 SMC 边界：
//! - `Diagnostics` 是公开平台 System，拥有运行时状态、跨模块编排与
//!   失败/上报策略。
//! - `reporting` 与 `recovery` 是一个 System 实例拥有的私有 Module；
//!   两个 Module 都不可导出，也不互相发现。
//! - 报告草稿构造、有界存储与 tracing 发射是由 reporting Module 选择并
//!   拥有的窄 Component。
//!
//! # 私有 Module 边界 compile-fail 测试
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
mod debug;
mod pending;
mod recovery;
mod report;
mod reporting;

use std::panic::Location;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::Error;

pub use config::{BacktracePolicy, DiagnosticsConfig};
pub(crate) use debug::debug_mode_from_env;
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
    debugging: debug::DebugModule,
}

/// 一个运行时的公开 Diagnostics System 句柄。
///
/// 句柄只暴露 System 契约。其 reporting 与 recovery Module、它们的
/// Component 以及可变状态对本运行时实例保持私有。
#[derive(Clone)]
pub struct Diagnostics {
    inner: Arc<DiagnosticsInner>,
}

impl Diagnostics {
    /// 使用指定配置创建相互隔离的诊断运行时实例。
    pub fn new(config: DiagnosticsConfig) -> Self {
        let runtime_id = NEXT_RUNTIME_ID.fetch_add(1, Ordering::Relaxed);
        let debug_mode = config.debug_mode;
        Self {
            inner: Arc::new(DiagnosticsInner {
                runtime_id,
                reporting: ReportingModule::new(&config),
                config,
                pending_failures: PendingFailureQueue::new(),
                recovery: Arc::new(recovery::RecoveryModule::new()),
                debugging: debug::DebugModule::new(debug_mode),
            }),
        }
    }

    /// 在最终责任边界上报一个应用持有的类型化错误。
    ///
    /// > **tracing 初始化顺序**:事件经 `tracing::event!` 发射,tracing 对每个
    /// > callsite 的 interest 首次注册即缓存。若在本进程设置 tracing
    /// > subscriber 之前调用过本方法,且此后事件不可见,调用
    /// > [`Self::rebuild_tracing_interest_cache`] 即可恢复。
    #[track_caller]
    pub fn report(&self, error: Error) -> ReportId {
        self.report_with_origin(error, ReportOrigin::application())
    }

    /// 重新评估 tracing 的进程级 callsite interest 缓存。
    ///
    /// tracing 对每个 callsite 的 `Interest` 首次注册即缓存;若某个 callsite 在
    /// 尚无任何 subscriber 的上下文下注册,会被缓存为 `never`,之后即使设置了
    /// subscriber 也收不到该 callsite 的事件(见 `diagnostics::reporting::emit`)。
    /// 应用在初始化 tracing subscriber 后调用本方法即可刷新全部 callsite 的
    /// interest,恢复诊断事件可见性。
    pub fn rebuild_tracing_interest_cache(&self) {
        tracing::callsite::rebuild_interest_cache();
    }

    /// 返回当前运行时是否启用了统一调试模式。
    #[inline(always)]
    pub fn debug_mode(&self) -> bool {
        self.inner.debugging.enabled()
    }

    /// 动态切换统一调试模式。
    ///
    /// 所有共享此 Diagnostics 实例的窗口会在下一次事件或帧边界观察到新值。
    pub fn set_debug_mode(&self, enabled: bool) {
        if self.inner.debugging.set_enabled(enabled) {
            tracing::info!(
                target: "uix::diagnostics",
                debug_event = "mode_changed",
                runtime_id = self.inner.runtime_id,
                enabled,
                "runtime debug mode changed"
            );
        }
    }

    /// 为同一批输入、状态变更与最终帧分配稳定关联身份。
    pub(crate) fn next_debug_correlation_id(&self) -> u64 {
        self.inner.debugging.next_correlation_id()
    }

    /// 返回按 `ReportId` 排序的不可变时间点快照。
    pub fn snapshot(&self) -> DiagnosticsSnapshot {
        self.inner.reporting.snapshot()
    }

    /// 返回原生回调用于把类型化失败投递给 owner 线程资源的运行时队列。
    /// 回调运行期间队列绝不调用用户代码。
    pub(crate) fn pending_failure_queue(&self) -> PendingFailureQueue {
        self.inner.pending_failures.clone()
    }

    /// 为一个精确类型化错误码注册恢复逻辑。
    ///
    /// 处理器仅在 [`Self::attempt_recovery`] 从安全 owner/调用线程被调用时
    /// 以稳定注册顺序同步运行。原生回调必须入队失败而不是调用用户代码。
    pub fn on_error<F>(&self, code: crate::core::Errc, handler: F) -> RecoverySubscription
    where
        F: Fn(&Error) -> RecoveryAction + Send + Sync + 'static,
    {
        self.inner.recovery.register(code, handler)
    }

    /// 尝试注册的恢复，而不隐式上报未处理错误。
    ///
    /// 调用方可继续传播该类型化错误，并且只在成为其最终责任边界时上报。
    pub fn attempt_recovery(&self, error: Error) -> RecoveryOutcome {
        self.inner.recovery.attempt(error)
    }

    /// 仅框架内部使用的上报入口，携带一个有界、类型化的来源。
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

    /// 安装绑定到本运行时的框架 panic hook。
    ///
    /// panic 永不吞没：先前 hook 总是被调用，因此默认输出与 unwind/abort
    /// 语义被保留。配置了崩溃目录时，转发前会原子写入一份有界崩溃报告。
    pub(crate) fn install_panic_hook(&self) {
        crash::install_panic_hook(self.clone());
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new(DiagnosticsConfig::default())
    }
}
