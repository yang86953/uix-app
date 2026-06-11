// ============================================================================
// diag/api.rs — Diagnostics 层的公共 API 出口
//
// 本文件定义 diag 层对外暴露的公共接口。其他层只能通过本文件
// 使用 diag 层的功能，禁止直接引用内部模块。
// ============================================================================

// ── 错误类型 ──
pub use crate::diag::error::{
    make_error, to_std_error_code, Error, ErrorSeverity, Errc,
};

// ── 结果类型 ──
pub use crate::diag::result::{
    collect_errors, collect_values, try_invoke, Result, ResultErrorExt, ResultExt, ResultVoidExt,
};

// ── 日志 ──
pub use crate::diag::log::{
    debug_fn, error_fn, fatal_fn, info_fn, trace_fn, warn_fn, CallbackSink, ConsoleSink, FileSink,
    Level, Logger, Record, Sink, log_error,
};

// ── 错误收集器 ──
pub use crate::diag::collector::{
    Collector, CollectorConfig, CollectorSnapshot, ScopedCollector,
};

// ── 致命错误处理 ──
pub use crate::diag::fatal::{
    abort_if_fatal, collect_or_abort, dump_crash_report, fatal_abort, install_fatal_handler,
};

// ── 恢复策略 ──
pub use crate::diag::recovery::{
    with_recovery, with_recovery_typed, retry, CircuitBreaker, CircuitState, ExponentialBackoffRetryPolicy,
    FilteredRetryPolicy, FixedRetryPolicy, RecoveryAction, RecoveryHandler, RetryPolicy,
};
