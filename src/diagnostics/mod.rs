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
//! use uix_app::diagnostics::reporting::ReportingModule;
//! ```
//!
//! ```compile_fail
//! use uix_app::diagnostics::recovery::RecoveryModule;
//! ```
//!
//! ```compile_fail
//! use uix_app::diagnostics::pending::PendingFailureQueue;
//! ```
//!
//! ```compile_fail
//! use uix_app::diagnostics::report::ReportOrigin;
//! ```
//!
//! ```compile_fail
//! use uix_app::diagnostics::config::DiagnosticsConfig;
//! ```
//!
//! ```compile_fail
//! use uix_app::diagnostics::crash::CrashReport;
//! ```

mod config;
mod crash;
mod debug;
mod pending;
mod recovery;
mod report;
mod reporting;
mod repro;
mod transient;

use std::panic::Location;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::Error;

pub use config::{BacktracePolicy, DiagnosticsConfig};
pub(crate) use debug::debug_mode_from_env;
pub(crate) use pending::{PendingFailureQueue, PendingFailureSource};
pub use recovery::{RecoveryAction, RecoveryOutcome, RecoverySubscription};
pub use reporting::ReportSubscription;

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
    repro: repro::ReproModule,
    transient: transient::TransientObservationModule,
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
                repro: repro::ReproModule::new(),
                transient: transient::TransientObservationModule::new(),
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

    /// 以错误值形态观察一个瞬态失败：等价 [`Self::observe_transient`]，
    /// 并把该错误标记为已经过诊断通道处置（不进入未处置落地观测）。
    pub fn observe_transient_error(
        &self,
        target: &'static str,
        reason: &'static str,
        error: &crate::core::Error,
    ) -> bool {
        error.mark_observed();
        self.observe_transient(target, reason, error.short_what())
    }

    /// 观察一个瞬态失败：按错误处置决策矩阵不进入报告存储。
    ///
    /// 适用于自愈重试、fallback 保持武装、高频平台噪声等设计内瞬态。同一
    /// `(target, reason)` 首条立即发射结构化事件，冷却窗口内（30 秒）的重复
    /// 观察被抑制并计数，窗口结束后的下一条携带累计抑制数——瞬态失败既从
    /// 第一条就可见，也不会刷屏。返回本次是否实际发射了事件，供测试与
    /// 调用方观测去重行为。
    pub fn observe_transient(
        &self,
        target: &'static str,
        reason: &'static str,
        detail: impl std::fmt::Display,
    ) -> bool {
        match self.inner.transient.observe(target, reason) {
            transient::TransientObservation::Emit {
                suppressed_in_window,
            } => {
                tracing::warn!(
                    target: "uix_app::diagnostics",
                    transient_target = target,
                    reason,
                    suppressed_in_window,
                    detail = %detail,
                    "transient failure observed (deduplicated)"
                );
                true
            }
            transient::TransientObservation::Suppressed => false,
        }
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
            self.inner.repro.record_mode_changed(enabled);
            tracing::info!(
                target: "uix_app::diagnostics",
                debug_event = "mode_changed",
                runtime_id = self.inner.runtime_id,
                enabled,
                "runtime debug mode changed"
            );
        }
    }

    /// 订阅每份报告的即时通知。
    ///
    /// 每当新报告入库（应用 [`Self::report`] 或框架内部上报），处理器在
    /// report 调用线程同步收到该 [`ErrorReport`]——宿主不依赖 tracing
    /// subscriber 也能在错误发生的第一时间感知。丢弃返回的 RAII 句柄即
    /// 停止通知；处理器内部的嵌套上报照常入库但不再次分发；处理器 panic
    /// 被隔离为限流 emergency 输出，不影响报告与其他订阅者。
    pub fn on_report<F>(&self, handler: F) -> ReportSubscription
    where
        F: Fn(&ErrorReport) + Send + Sync + 'static,
    {
        self.inner.reporting.subscribe_report(handler)
    }

    /// 为同一批输入、状态变更与最终帧分配稳定关联身份。
    pub(crate) fn next_debug_correlation_id(&self) -> u64 {
        self.inner.debugging.next_correlation_id()
    }

    /// 写出一份有界、脱敏的运行时复现清单。
    ///
    /// 清单只包含固定事件类型、关联身份、阶段耗时、窗口数值状态与错误码；
    /// 不包含用户文本、窗口标题、资源标识、环境变量或本地路径。即使当前未开启
    /// debug 也可调用，此时仍会导出平台版本与最近错误码。
    pub fn write_debug_repro_manifest(
        &self,
        directory: impl AsRef<std::path::Path>,
    ) -> Result<std::path::PathBuf, Error> {
        let snapshot = self.inner.repro.capture(
            repro::CaptureReason::Manual,
            self.inner.runtime_id,
            self.debug_mode(),
        );
        crash::write_text_atomic(
            directory.as_ref(),
            "repro",
            snapshot.captured_at(),
            snapshot.runtime_id(),
            &repro::render(&snapshot),
        )
    }

    pub(crate) fn record_debug_input(
        &self,
        window_id: crate::core::WindowId,
        correlation_id: u64,
        event_type: &'static str,
    ) {
        if !self.debug_mode() {
            return;
        }
        self.inner
            .repro
            .record_input(window_id, correlation_id, event_type);
    }

    pub(crate) fn record_debug_hover_changed(
        &self,
        window_id: crate::core::WindowId,
        correlation_id: u64,
    ) {
        if !self.debug_mode() {
            return;
        }
        self.inner
            .repro
            .record_hover_changed(window_id, correlation_id);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_debug_frame(
        &self,
        window_id: crate::core::WindowId,
        correlation_id: Option<u64>,
        width: u32,
        height: u32,
        frame: std::time::Duration,
        layout: std::time::Duration,
        render: std::time::Duration,
        submit: std::time::Duration,
        present: std::time::Duration,
        dirty_full: bool,
        dirty_area_ratio: f64,
        animation_count: u32,
        invalidation_count: usize,
        reconcile_ran: bool,
        tree_version_delta: u64,
        invalidation_source: &'static str,
    ) {
        if !self.debug_mode() {
            return;
        }
        self.inner.repro.record_frame(
            window_id,
            correlation_id,
            width,
            height,
            frame,
            layout,
            render,
            submit,
            present,
            dirty_full,
            dirty_area_ratio,
            animation_count,
            invalidation_count,
            reconcile_ran,
            tree_version_delta,
            invalidation_source,
        );
    }

    /// 返回按 `ReportId` 排序的不可变时间点快照。
    pub fn snapshot(&self) -> DiagnosticsSnapshot {
        let mut snapshot = self.inner.reporting.snapshot();
        // 同一运行时的瞬态观察量级并入快照，宿主可回看观察通道活动。
        let (total, suppressed) = self.inner.transient.counts();
        snapshot.total_transient_observations = total;
        snapshot.suppressed_transient_observations = suppressed;
        snapshot
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
        // 进入报告通道即完成该错误（及其原因链）的诊断处置。
        error.mark_observed();
        let report_site = Location::caller();
        let id = self.inner.reporting.report(
            self.inner.runtime_id,
            self.inner.config.backtrace,
            &error,
            origin,
            report_site,
        );
        self.inner.repro.record_error(id, &error);
        id
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

    pub(crate) fn write_debug_repro_manifest_for_panic(
        &self,
        directory: &std::path::Path,
    ) -> Result<std::path::PathBuf, Error> {
        let snapshot = self
            .inner
            .repro
            .try_capture_for_panic(self.inner.runtime_id, self.debug_mode());
        crash::write_text_atomic(
            directory,
            "repro",
            snapshot.captured_at(),
            snapshot.runtime_id(),
            &repro::render(&snapshot),
        )
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new(DiagnosticsConfig::default())
    }
}

/// 无诊断句柄层（UI 树、adapter teardown 等 SMC 边界层）的显式边界观察入口。
///
/// 这些层按架构边界不持有 [`Diagnostics`] 句柄（诊断属于 app 组合根），
/// 失败保留 typed 结构化日志观察；本入口把该决策显式化为命名调用，取代
/// 裸 `tracing::error!`，使错误处置类别可从调用点直接判读。
pub fn observe_boundary_error(layer: &'static str, error: &crate::core::Error) {
    // 边界观察同样完成诊断处置：错误已被显式观察而非被吞。
    error.mark_observed();
    tracing::error!(
        target: "uix_app::diagnostics",
        boundary_layer = layer,
        error = %error.short_what(),
        "boundary failure observed outside diagnostics ownership"
    );
}

/// 契约破坏 panic 的统一入口：文案带稳定前缀，便于日志与崩溃报告检索。
///
/// 按错误处置决策矩阵，开发者契约或内部不变量破坏选择 panic 终止；调用
/// 方应在函数文档中声明契约，文案只报告事实与位置，不携带业务标识。
#[macro_export]
macro_rules! uix_contract_violation {
    ($($arg:tt)*) => {
        panic!("[uix-contract] {}", format_args!($($arg)*))
    };
}
