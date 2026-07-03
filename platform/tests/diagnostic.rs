//! diagnostic — 诊断系统测试（Collector / RetryPolicy / CircuitBreaker / Middleware）。

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use uix_platform::diagnostic::{
    retry, with_recovery, with_recovery_typed, CircuitBreaker, CircuitState, Collector,
    CollectorConfig, CollectorSnapshot, ExponentialBackoffRetryPolicy, FilteredRetryPolicy,
    FixedRetryPolicy, LogMiddleware, Middleware, MiddlewareContext, MiddlewarePipeline,
    RecoveryAction, RecoveryHandler, RetryMiddleware, RetryPolicy, ScopedCollector,
};
use uix_platform::error::{Errc, Error};

#[test]
fn collector_config_default() {
    let cfg = CollectorConfig::default();
    assert_eq!(cfg.max_errors, 4096);
    assert!(cfg.auto_log);
    assert!(!cfg.deduplicate);
}

/// 所有依赖 Collector 单例的测试合并在一个函数中运行，
/// 避免并行测试之间相互干扰（Collector 是全局单例）。
#[test]
fn collector_sequential() {
    let c = Collector::instance();
    // 不依赖 clear()，因为并行测试也在随时收集错误，
    // 所有断言使用相对值或存在性检查而非精确计数。

    // ── configure / get_config 读写 ──
    c.configure(CollectorConfig {
        max_errors: 100,
        auto_log: false,
        auto_log_level: uix_platform::log::Level::Warn,
        deduplicate: false,
    });
    let cfg = c.get_config();
    assert_eq!(cfg.max_errors, 100);
    assert!(!cfg.auto_log);
    assert!(!cfg.deduplicate);

    // ── configure 覆盖 ──
    c.configure(CollectorConfig {
        max_errors: 20,
        auto_log: true,
        auto_log_level: uix_platform::log::Level::Warn,
        deduplicate: true,
    });
    let cfg2 = c.get_config();
    assert_eq!(cfg2.max_errors, 20);
    assert!(cfg2.auto_log);
    assert!(cfg2.deduplicate);

    // 恢复配置
    c.configure(CollectorConfig {
        max_errors: 1000,
        auto_log: false,
        auto_log_level: uix_platform::log::Level::Warn,
        deduplicate: false,
    });

    // ── collect / report / collect_exception / collect_fn ──
    let before = c.total_collected();
    c.collect(Error::new(Errc::NotFound, "not found"));
    assert!(c.total_collected() > before);
    assert!(c.has_errors());

    c.report(Errc::IoError, "io issue");
    c.collect_exception("exception", Errc::PlatformError);
    c.collect_fn(|| Error::new(Errc::Unknown, "fn error"));
    assert_eq!(c.total_collected(), before + 4);

    // ── errors / errors_by_code / errors_in_window / errors_if ──
    let errors = c.errors();
    assert!(errors.iter().any(|e| e.message() == "not found"));

    let nf = c.errors_by_code(Errc::NotFound);
    assert!(nf.iter().any(|e| e.message() == "not found"));

    let all = c.errors_in_window(SystemTime::UNIX_EPOCH, SystemTime::now());
    assert!(!all.is_empty());

    let found = c.errors_if(|e| e.code() == Errc::NotFound);
    assert!(found.iter().any(|e| e.message() == "not found"));

    // ── count_by_code / count_by_category ──
    let counts = c.count_by_code();
    assert!(counts.contains_key(&Errc::NotFound));

    let cats = c.count_by_category();
    assert!(cats.contains_key("general"), "categories: {:?}", cats);

    // ── snapshot ──
    let snap = c.snapshot();
    assert_eq!(snap.total_collected, c.total_collected());
    assert_eq!(snap.stored_count, c.stored_count());
    let snap_counts = snap.count_by_code();
    assert!(snap_counts.contains_key(&Errc::NotFound));

    // ── dump / summary ──
    let dump = c.dump();
    assert!(!dump.is_empty());

    let s = c.summary();
    assert!(s.contains("errors collected"));

    // ── clear_by_code ──
    c.clear_by_code(Errc::NotFound);
    for e in c.errors() {
        assert_ne!(e.code(), Errc::NotFound);
    }

    // ── clear_before ──
    let before_tp = SystemTime::now();
    c.collect(Error::new(Errc::None, "after"));
    c.clear_before(before_tp);
    assert!(c.stored_count() >= 1);

    // ── clear 清空 ──
    c.clear();
    assert!(!c.has_errors());
    assert_eq!(c.stored_count(), 0);

    // ── deduplication ──
    c.set_deduplicate(true);
    c.collect(Error::new(Errc::AccessDenied, "dup"));
    c.collect(Error::new(Errc::AccessDenied, "dup"));
    c.collect(Error::new(Errc::AccessDenied, "dup"));
    assert_eq!(c.stored_count(), 1);
    for e in c.errors() {
        assert_eq!(e.code(), Errc::AccessDenied);
    }
    c.set_deduplicate(false);

    // ── on_collect token 注册/移除 ──
    let token = c.on_collect(|_: &Error| {});
    assert!(c.remove_callback(token));
    assert!(!c.remove_callback(token));

    // ── set_max_errors ──
    c.clear();
    c.set_max_errors(2);
    c.collect(Error::new(Errc::BadWeakPointer, "a"));
    c.collect(Error::new(Errc::BadWeakPointer, "b"));
    c.collect(Error::new(Errc::BadWeakPointer, "c"));
    assert_eq!(c.stored_count(), 2);

    // ── set_auto_log ──
    c.set_auto_log(false, uix_platform::log::Level::Info);
    let cfg3 = c.get_config();
    assert!(!cfg3.auto_log);

    // ── scoped_collector ──
    c.clear();
    {
        let mut scoped = ScopedCollector::new(Collector::instance());
        scoped.collect(Error::new(Errc::DeadlockDetected, "scoped1"));
        scoped.collect(Error::new(Errc::DeadlockDetected, "scoped2"));
        assert_eq!(Collector::instance().stored_count(), 0);
        scoped.flush();
        assert_eq!(Collector::instance().stored_count(), 2);
    }

    // ── scoped discard ──
    c.clear();
    {
        let mut scoped = ScopedCollector::new(Collector::instance());
        scoped.collect(Error::new(Errc::DeadlockDetected, "discarded"));
        scoped.discard();
        scoped.flush();
        assert_eq!(Collector::instance().stored_count(), 0);
    }

    // ── scoped default ──
    let _ = ScopedCollector::default();

    // ── scoped drop flushes ──
    c.clear();
    {
        let mut scoped = ScopedCollector::new(Collector::instance());
        scoped.collect(Error::new(Errc::DeadlockDetected, "auto_flush"));
    }
    assert_eq!(Collector::instance().stored_count(), 1);

    // ── 最终清理 ──
    c.clear();
}

#[test]
fn collector_snapshot_display() {
    let snap = CollectorSnapshot {
        timestamp: SystemTime::now(),
        total_collected: 3,
        stored_count: 2,
        errors: vec![Error::new(Errc::None, "test error")],
    };
    let s = snap.to_string();
    assert!(s.contains("Total collected: 3"));
    assert!(s.contains("Stored: 2"));
}

#[test]
fn fixed_retry_policy_basic() {
    let policy = FixedRetryPolicy::new(3, 1);
    assert!(policy.should_retry(0, &Error::new(Errc::None, "")));
    assert!(policy.should_retry(2, &Error::new(Errc::None, "")));
    assert!(!policy.should_retry(3, &Error::new(Errc::None, "")));
}

#[test]
fn fixed_retry_policy_delay() {
    let policy = FixedRetryPolicy::new(3, 50);
    assert_eq!(policy.delay(0), Duration::from_millis(50));
    assert_eq!(policy.delay(5), Duration::from_millis(50));
}

#[test]
fn fixed_retry_policy_describe() {
    let policy = FixedRetryPolicy::new(2, 100);
    let d = policy.describe();
    assert!(d.contains("max=2"));
    assert!(d.contains("delay=100ms"));
}

#[test]
fn fixed_retry_policy_max_retries() {
    let mut policy = FixedRetryPolicy::new(1, 0);
    assert_eq!(policy.max_retries(), 1);
    policy.set_max_retries(5);
    assert_eq!(policy.max_retries(), 5);
}

#[test]
fn exponential_backoff_default() {
    let d1 = ExponentialBackoffRetryPolicy::default().describe();
    let d2 = ExponentialBackoffRetryPolicy::default().describe();
    assert_eq!(d1, d2);
}

#[test]
fn exponential_backoff_should_retry() {
    let policy = ExponentialBackoffRetryPolicy::new(2, 1, 100, 2.0, 0.0);
    assert!(policy.should_retry(0, &Error::new(Errc::None, "")));
    assert!(policy.should_retry(1, &Error::new(Errc::None, "")));
    assert!(!policy.should_retry(2, &Error::new(Errc::None, "")));
}

#[test]
fn exponential_backoff_delay_increases() {
    let policy = ExponentialBackoffRetryPolicy::new(5, 10, 1000, 2.0, 0.0);
    let d0 = policy.delay(0);
    let d1 = policy.delay(1);
    assert!(d1 > d0);
}

#[test]
fn exponential_backoff_delay_capped() {
    let policy = ExponentialBackoffRetryPolicy::new(5, 10, 100, 10.0, 0.0);
    let d = policy.delay(3);
    assert!(d <= Duration::from_millis(100));
}

#[test]
fn exponential_backoff_describe() {
    let policy = ExponentialBackoffRetryPolicy::new(3, 50, 5000, 2.0, 0.1);
    let d = policy.describe();
    assert!(d.contains("exponential_backoff"));
}

#[test]
fn filtered_retry_policy_filters_by_code() {
    let inner = FixedRetryPolicy::new(3, 0);
    let policy = FilteredRetryPolicy::new(Box::new(inner), vec![Errc::NotFound]);
    assert!(policy.should_retry(0, &Error::new(Errc::NotFound, "")));
    assert!(!policy.should_retry(0, &Error::new(Errc::Unknown, "")));
}

#[test]
fn filtered_retry_policy_delegates_delay() {
    let inner = FixedRetryPolicy::new(3, 42);
    let policy = FilteredRetryPolicy::new(Box::new(inner), vec![]);
    assert_eq!(policy.delay(0), Duration::from_millis(42));
}

#[test]
fn filtered_retry_policy_describe() {
    let inner = FixedRetryPolicy::new(1, 0);
    let policy = FilteredRetryPolicy::new(Box::new(inner), vec![]);
    let d = policy.describe();
    assert!(d.contains("filtered"));
}

#[test]
fn circuit_breaker_initial_state_closed() {
    let cb = CircuitBreaker::new(3, Duration::from_secs(10));
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.failure_count(), 0);
    assert_eq!(cb.success_count(), 0);
    assert_eq!(cb.rejected_count(), 0);
}

#[test]
fn circuit_breaker_try_call_closed() {
    let cb = CircuitBreaker::new(3, Duration::from_secs(10));
    assert!(cb.try_call());
}

#[test]
fn circuit_breaker_opens_on_threshold() {
    let cb = CircuitBreaker::new(3, Duration::from_secs(10));
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Closed);
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
}

#[test]
fn circuit_breaker_rejects_when_open() {
    let cb = CircuitBreaker::new(1, Duration::from_secs(10));
    cb.record_failure();
    assert!(!cb.try_call());
    assert_eq!(cb.rejected_count(), 1);
}

#[test]
fn circuit_breaker_success_closes_half_open() {
    let cb = CircuitBreaker::new(1, Duration::from_secs(1));
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
    std::thread::sleep(Duration::from_millis(1100));
    assert_eq!(cb.state(), CircuitState::HalfOpen);
    assert!(cb.try_call());
    cb.record_success();
    assert_eq!(cb.state(), CircuitState::Closed);
}

#[test]
fn circuit_breaker_reset() {
    let cb = CircuitBreaker::new(1, Duration::from_secs(10));
    cb.record_failure();
    cb.record_failure();
    cb.reset();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.failure_count(), 0);
    assert_eq!(cb.rejected_count(), 0);
}

#[test]
fn circuit_breaker_threshold() {
    let cb = CircuitBreaker::new(5, Duration::from_secs(30));
    assert_eq!(cb.threshold(), 5);
    cb.set_threshold(2);
    assert_eq!(cb.threshold(), 2);
}

#[test]
fn circuit_breaker_recovery_timeout() {
    let cb = CircuitBreaker::new(3, Duration::from_secs(60));
    let to = cb.recovery_timeout();
    assert_eq!(to.as_secs(), 60);
    cb.set_recovery_timeout(Duration::from_secs(30));
    assert_eq!(cb.recovery_timeout().as_secs(), 30);
}

#[test]
fn circuit_breaker_default() {
    let cb = CircuitBreaker::default();
    assert_eq!(cb.threshold(), 5);
    assert_eq!(cb.recovery_timeout().as_secs(), 30);
}

#[test]
fn circuit_breaker_describe() {
    let cb = CircuitBreaker::new(3, Duration::from_secs(10));
    let d = cb.describe();
    assert!(d.contains("circuit_breaker"));
}

#[test]
fn circuit_state_display() {
    assert_eq!(format!("{}", CircuitState::Closed), "closed");
    assert_eq!(format!("{}", CircuitState::Open), "open");
    assert_eq!(format!("{}", CircuitState::HalfOpen), "half_open");
}

#[test]
fn circuit_state_from_u64() {
    let closed: CircuitState = 0u64.into();
    assert_eq!(closed, CircuitState::Closed);
    let open: CircuitState = 1u64.into();
    assert_eq!(open, CircuitState::Open);
    let half: CircuitState = 2u64.into();
    assert_eq!(half, CircuitState::HalfOpen);
    let unknown: CircuitState = 99u64.into();
    assert_eq!(unknown, CircuitState::Open);
}

#[test]
fn recovery_action_variants() {
    assert_eq!(RecoveryAction::Retry as u8, 0);
    assert_eq!(RecoveryAction::Fallback as u8, 1);
    assert_eq!(RecoveryAction::Abort as u8, 2);
    assert_eq!(RecoveryAction::Skip as u8, 3);
}

#[test]
fn recovery_handler_success() {
    let policy = FixedRetryPolicy::new(3, 0);
    let handler = RecoveryHandler::new(Box::new(policy));
    let result = handler.execute(|| Ok(()));
    assert!(result.is_ok());
}

#[test]
fn recovery_handler_success_on_retry() {
    let policy = FixedRetryPolicy::new(3, 0);
    let handler = RecoveryHandler::new(Box::new(policy));
    let attempt = AtomicI32::new(0);
    let result = handler.execute(|| {
        let n = attempt.fetch_add(1, Ordering::SeqCst);
        if n < 2 {
            Err(Error::new(Errc::None, "not yet"))
        } else {
            Ok(())
        }
    });
    assert!(result.is_ok());
}

#[test]
fn recovery_handler_fallback_executed() {
    let policy = FixedRetryPolicy::new(1, 0);
    let mut handler = RecoveryHandler::new(Box::new(policy));
    let fallback_called = Arc::new(AtomicBool::new(false));
    let fb = fallback_called.clone();
    handler.set_fallback(move |_: &Error| {
        fb.store(true, Ordering::SeqCst);
        Ok(())
    });
    let result = handler.execute(|| Err(Error::new(Errc::None, "always fail")));
    assert!(result.is_ok());
    assert!(fallback_called.load(Ordering::SeqCst));
}

#[test]
fn recovery_handler_fallback_not_called_on_success() {
    let policy = FixedRetryPolicy::new(1, 0);
    let mut handler = RecoveryHandler::new(Box::new(policy));
    let fallback_called = Arc::new(AtomicBool::new(false));
    let fb = fallback_called.clone();
    handler.set_fallback(move |_: &Error| {
        fb.store(true, Ordering::SeqCst);
        Ok(())
    });
    let result = handler.execute(|| Ok(()));
    assert!(result.is_ok());
    assert!(!fallback_called.load(Ordering::SeqCst));
}

#[test]
fn recovery_handler_no_fallback_returns_error() {
    let policy = FixedRetryPolicy::new(1, 0);
    let handler = RecoveryHandler::new(Box::new(policy));
    let result = handler.execute(|| Err(Error::new(Errc::None, "fail")));
    assert!(result.is_err());
}

#[test]
fn recovery_handler_on_retry_called() {
    let policy = FixedRetryPolicy::new(1, 0);
    let mut handler = RecoveryHandler::new(Box::new(policy));
    let retry_count = Arc::new(AtomicI32::new(0));
    let rc = retry_count.clone();
    handler.set_on_retry(move |_: &Error, _: usize| {
        rc.fetch_add(1, Ordering::SeqCst);
    });
    let _ = handler.execute(|| Err(Error::new(Errc::None, "fail")));
    // max_retries=1 表示最多重试 1 次，总共执行 2 次，
    // 每次失败都调用 on_retry，所以共 2 次
    assert_eq!(retry_count.load(Ordering::SeqCst), 2);
}

#[test]
fn with_recovery_success() {
    let policy = FixedRetryPolicy::new(2, 0);
    let result = with_recovery(
        Box::new(policy),
        || Ok(()),
        None::<std::boxed::Box<dyn Fn(&Error) -> std::result::Result<(), Error> + Send + Sync>>,
    );
    assert!(result.is_ok());
}

#[test]
fn with_recovery_fallback() {
    let policy = FixedRetryPolicy::new(1, 0);
    let result = with_recovery(
        Box::new(policy),
        || Err(Error::new(Errc::None, "fail")),
        Some(Box::new(|_: &Error| Ok(()))),
    );
    assert!(result.is_ok());
}

#[test]
fn retry_success() {
    let result = retry(3, || Ok(()), 0);
    assert!(result.is_ok());
}

#[test]
fn retry_fail_exhausted() {
    let result = retry(2, || Err(Error::new(Errc::None, "fail")), 0);
    assert!(result.is_err());
}

#[test]
fn retry_success_eventually() {
    let attempt = AtomicI32::new(0);
    let result = retry(
        3,
        || {
            let n = attempt.fetch_add(1, Ordering::SeqCst);
            if n < 2 {
                Err(Error::new(Errc::None, "retry"))
            } else {
                Ok(())
            }
        },
        0,
    );
    assert!(result.is_ok());
}

#[test]
fn with_recovery_typed_success() {
    let policy = FixedRetryPolicy::new(2, 0);
    let result: Result<i32, Error> = with_recovery_typed(
        Box::new(policy),
        || Ok(42),
        None::<std::boxed::Box<dyn Fn(&Error) -> std::result::Result<i32, Error> + Send + Sync>>,
    );
    assert_eq!(result.unwrap(), 42);
}

#[test]
fn with_recovery_typed_fallback() {
    let policy = FixedRetryPolicy::new(1, 0);
    let result: Result<i32, Error> = with_recovery_typed(
        Box::new(policy),
        || Err(Error::new(Errc::None, "fail")),
        Some(Box::new(|_: &Error| Ok(-1))),
    );
    assert_eq!(result.unwrap(), -1);
}

#[test]
fn with_recovery_typed_fail() {
    let policy = FixedRetryPolicy::new(1, 0);
    let result: Result<i32, Error> = with_recovery_typed(
        Box::new(policy),
        || Err(Error::new(Errc::None, "fail")),
        None::<std::boxed::Box<dyn Fn(&Error) -> std::result::Result<i32, Error> + Send + Sync>>,
    );
    assert!(result.is_err());
}

#[test]
fn middleware_pipeline_empty() {
    let pipeline = MiddlewarePipeline::new();
    assert!(pipeline.is_empty());
    assert_eq!(pipeline.len(), 0);
}

#[test]
fn middleware_pipeline_execute_empty() {
    let pipeline = MiddlewarePipeline::new();
    let mut ctx = MiddlewareContext::default();
    let executed = Arc::new(AtomicBool::new(false));
    let e = executed.clone();
    pipeline.execute(&mut ctx, move |_: &mut MiddlewareContext| {
        e.store(true, Ordering::SeqCst);
    });
    assert!(executed.load(Ordering::SeqCst));
}

#[test]
fn middleware_pipeline_add_and_len() {
    let mut pipeline = MiddlewarePipeline::new();
    pipeline.add(LogMiddleware);
    pipeline.add(LogMiddleware);
    assert_eq!(pipeline.len(), 2);
    assert!(!pipeline.is_empty());
}

#[test]
fn middleware_pipeline_clear() {
    let mut pipeline = MiddlewarePipeline::new();
    pipeline.add(LogMiddleware);
    pipeline.clear();
    assert!(pipeline.is_empty());
}

#[test]
fn log_middleware_name() {
    let mw = LogMiddleware;
    assert_eq!(mw.name(), "LogMiddleware");
}

#[test]
fn retry_middleware_name() {
    let mw = RetryMiddleware::new(3);
    assert_eq!(mw.name(), "RetryMiddleware");
}

#[test]
fn retry_middleware_success_first_try() {
    let mw = RetryMiddleware::new(3);
    let mut ctx = MiddlewareContext {
        service_name: "svc".into(),
        operation: "op".into(),
        ..Default::default()
    };
    mw.handle(&mut ctx, &mut |c: &mut MiddlewareContext| {
        c.succeeded = true;
    });
    assert!(ctx.succeeded);
    assert_eq!(ctx.retry_count, 0);
}

#[test]
fn retry_middleware_success_on_retry() {
    let mw = RetryMiddleware::new(3);
    let attempt = AtomicI32::new(0);
    let mut ctx = MiddlewareContext {
        service_name: "svc".into(),
        operation: "op".into(),
        ..Default::default()
    };
    mw.handle(&mut ctx, &mut |c: &mut MiddlewareContext| {
        if attempt.fetch_add(1, Ordering::SeqCst) == 0 {
            c.succeeded = false;
            c.error_message = "fail".into();
        } else {
            c.succeeded = true;
        }
    });
    assert!(ctx.succeeded);
}

#[test]
fn retry_middleware_exhausted() {
    let mw = RetryMiddleware::new(1);
    let mut ctx = MiddlewareContext {
        service_name: "svc".into(),
        operation: "op".into(),
        ..Default::default()
    };
    mw.handle(&mut ctx, &mut |c: &mut MiddlewareContext| {
        c.succeeded = false;
        c.error_message = "always fail".into();
    });
    assert!(!ctx.succeeded);
}

#[test]
fn middleware_context_default() {
    let ctx = MiddlewareContext::default();
    assert_eq!(ctx.service_name, "");
    assert_eq!(ctx.operation, "");
    assert_eq!(ctx.status_code, 0);
    assert!(ctx.succeeded);
    assert_eq!(ctx.retry_count, 0);
}

#[test]
fn middleware_pipeline_multiple_middleware() {
    let mut pipeline = MiddlewarePipeline::new();
    pipeline.add(RetryMiddleware::new(1));
    pipeline.add(LogMiddleware);
    assert_eq!(pipeline.len(), 2);
    let mut ctx = MiddlewareContext::default();
    let chain = Arc::new(AtomicBool::new(false));
    let c = chain.clone();
    pipeline.execute(&mut ctx, move |_: &mut MiddlewareContext| {
        c.store(true, Ordering::SeqCst);
    });
    assert!(chain.load(Ordering::SeqCst));
}

// ════════════════════════════════════════════════════════════════════════════
// fatal — 崩溃处理（depend on Collector singleton → sequential）
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn fatal_sequential() {
    let c = Collector::instance();

    // ── dump_crash_report 生成文件 ──
    let crash_path = std::path::Path::new("uix_crash.log");
    let _ = std::fs::remove_file(crash_path);
    uix_platform::diagnostic::dump_crash_report();
    assert!(
        crash_path.exists(),
        "uix_crash.log should exist after dump_crash_report"
    );
    let content = std::fs::read_to_string(crash_path).unwrap_or_default();
    assert!(
        content.contains("UIX CRASH REPORT"),
        "should contain report header"
    );
    assert!(content.contains("Timestamp"), "should contain timestamp");
    let _ = std::fs::remove_file(crash_path);

    // ── abort_if_fatal 非 fatal 不 abort ──
    let err = Error::new(Errc::NotFound, "non-fatal test");
    let result = uix_platform::diagnostic::abort_if_fatal(err);
    assert_eq!(result.code(), Errc::NotFound);

    // ── collect_or_abort 非 fatal 只收集 ──
    let before_fatal = c.total_collected();
    uix_platform::diagnostic::collect_or_abort(Error::warn(Errc::Timeout, "collect only"));
    assert!(
        c.total_collected() > before_fatal,
        "collect_or_abort should collect the error"
    );

    // ── install_fatal_handler 能安全调用（Once 保护） ──
    uix_platform::diagnostic::install_fatal_handler();
    // 第二次调用应静默成功（call_once 保护）
    uix_platform::diagnostic::install_fatal_handler();
}

#[test]
fn fatal_abort_if_fatal_severity_info() {
    let err = Error::info(Errc::None, "info severity");
    let result = uix_platform::diagnostic::abort_if_fatal(err);
    assert_eq!(result.code(), Errc::None);
    assert_eq!(result.severity(), uix_platform::error::ErrorSeverity::Info);
}

#[test]
fn fatal_abort_if_fatal_severity_warning() {
    let err = Error::warn(Errc::NotFound, "warning");
    let result = uix_platform::diagnostic::abort_if_fatal(err);
    assert_eq!(
        result.severity(),
        uix_platform::error::ErrorSeverity::Warning
    );
}

#[test]
fn fatal_collect_or_abort_collects_specific_error() {
    let c = Collector::instance();
    let _before2 = c.total_collected();
    uix_platform::diagnostic::collect_or_abort(Error::invalid_arg(
        "collected_by_collect_or_abort_test",
    ));
    let found = c.errors_if(|e| e.message().contains("collected_by_collect_or_abort_test"));
    assert!(
        !found.is_empty(),
        "collect_or_abort should have collected the error"
    );
}
