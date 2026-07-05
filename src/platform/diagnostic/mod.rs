// ============================================================================
// platform/src/diagnostic/mod.rs — 诊断系统入口
//
// 提供跨层共享的诊断基础设施：错误收集、崩溃处理、恢复策略、时间戳、中间件。
// 所有层（platform / graphics / ui / app / services）均可使用。
//
// 子模块：
//   timestamp.rs       — 轻量时间戳
//   collector.rs       — 错误收集器（含 ScopedCollector）
//   fatal.rs           — 崩溃处理（panic hook、crash dump）
//   recovery_policy.rs — 重试策略、断路器
//   recovery.rs        — 恢复执行逻辑
//   middleware.rs      — 中间件管道
// ============================================================================

pub mod collector;
pub mod fatal;
pub mod middleware;
pub mod recovery;
pub mod recovery_policy;
pub mod timestamp;

pub use collector::{Collector, CollectorConfig, CollectorSnapshot, ScopedCollector};
pub use fatal::{
    abort_if_fatal, collect_or_abort, dump_crash_report, fatal_abort, install_fatal_handler,
};
pub use middleware::{
    LogMiddleware, Middleware, MiddlewareContext, MiddlewarePipeline, RetryMiddleware,
};
pub use recovery::{retry, with_recovery, with_recovery_typed, RecoveryHandler};
pub use recovery_policy::*;
pub use timestamp::Timestamp;
