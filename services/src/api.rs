// ============================================================================
// services/api.rs — Services 层的公共 API 出口
//
// 本文件定义 services 层对外暴露的公共接口。其他层只能通过本文件
// 使用 services 层的功能，禁止直接引用内部模块。
// ============================================================================

// ── 业务服务 ──
pub use crate::file_service::FileService;
pub use crate::middleware::{
    LogMiddleware, Middleware, MiddlewareContext, MiddlewarePipeline, RetryMiddleware,
};
pub use crate::notification_service::{NotificationLevel, NotificationService, ToastEntry};
pub use crate::settings::SettingsService;

// ── 日志 ──
pub use crate::log::{
    debug_fn, error_fn, fatal_fn, info_fn, trace_fn, warn_fn, log_error, CallbackSink, ConsoleSink,
    FileSink, Level, Logger, Record, Sink,
};

// ── 错误收集器 ──
pub use crate::collector::{
    Collector, CollectorConfig, CollectorSnapshot, ScopedCollector,
};

// ── 致命错误处理 ──
pub use crate::fatal::{
    abort_if_fatal, collect_or_abort, dump_crash_report, fatal_abort, install_fatal_handler,
};

// ── 恢复策略 ──
pub use crate::recovery::{
    retry, with_recovery, with_recovery_typed, CircuitBreaker, CircuitState,
    ExponentialBackoffRetryPolicy, FilteredRetryPolicy, FixedRetryPolicy, RecoveryAction,
    RecoveryHandler, RetryPolicy,
};
