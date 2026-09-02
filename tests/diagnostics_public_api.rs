use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tracing::span::{Attributes, Id, Record};
use tracing::subscriber::Interest;
use tracing::{Event, Metadata, Subscriber};
use uix::core::{Errc, Error, ErrorSeverity};
use uix::diagnostics::{
    BacktracePolicy, Diagnostics, DiagnosticsConfig, RecoveryAction, RecoveryOutcome,
};

/// 全局默认 subscriber:`register_callsite` 恒返回 `Interest::sometimes()`。
///
/// tracing 的 callsite interest 是进程级、首次注册即缓存的(见 tracing-core
/// `DefaultCallsite::register`)。若一个 callsite 首次在"无任何 subscriber 的空
/// dispatch"下注册,`NoSubscriber::register_callsite` 会返回 `Interest::never()`
/// 并被永久缓存,之后所有 `with_default` 的 subscriber 都收不到该 callsite 的
/// 事件。测试并行时,无 `with_default` 的测试(如 `report_is_retained_...`、
/// `concurrent_reports_...` 的 worker 线程)可能先注册 `emit_event` 的共享
/// callsite,导致 `reentrant_report_...` 偶发收不到事件而失败。安装一个全局
/// 默认 subscriber 后,任何线程首次注册 callsite 时都会经过它,interest 恒为
/// `sometimes`,从而消除该竞态。
struct GlobalNoop;

impl Subscriber for GlobalNoop {
    fn register_callsite(&self, _metadata: &Metadata<'_>) -> Interest {
        Interest::sometimes()
    }

    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, _event: &Event<'_>) {}

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

/// 进程内只安装一次全局默认 subscriber。`set_global_default` 重复调用返回
/// `Err`,OnceLock 保证只尝试一次。
fn install_global_default_subscriber() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let _ = tracing::subscriber::set_global_default(GlobalNoop);
    });
}

#[test]
fn report_is_retained_without_a_subscriber_and_snapshot_is_immutable() {
    install_global_default_subscriber();
    let diagnostics = Diagnostics::new(
        DiagnosticsConfig::default()
            .report_capacity(2)
            .backtrace(BacktracePolicy::Disabled),
    );

    let first_id = diagnostics.report(Error::new(Errc::IoError, "first"));
    let first_snapshot = diagnostics.snapshot();
    assert_eq!(first_snapshot.total_reports(), 1);
    assert_eq!(first_snapshot.reports().len(), 1);
    assert_eq!(first_snapshot.reports()[0].id(), first_id);
    assert!(first_snapshot.reports()[0].event_emitted());

    diagnostics.report(Error::warn(Errc::Timeout, "second"));
    diagnostics.report(Error::fatal(Errc::InvalidState, "third"));

    assert_eq!(first_snapshot.total_reports(), 1);
    assert_eq!(first_snapshot.reports().len(), 1);

    let latest = diagnostics.snapshot();
    assert_eq!(latest.total_reports(), 3);
    assert_eq!(latest.evicted_reports(), 1);
    assert_eq!(latest.reports().len(), 2);
    assert!(latest.reports()[0].id() < latest.reports()[1].id());
    assert_eq!(latest.reports()[1].severity(), ErrorSeverity::Fatal);
}

#[test]
fn reports_are_sanitized_and_cause_depth_is_bounded() {
    install_global_default_subscriber();
    let diagnostics =
        Diagnostics::new(DiagnosticsConfig::default().backtrace(BacktracePolicy::Disabled));
    let long = format!("{}\nsecret\t{}", "界".repeat(1_000), "\0");
    diagnostics.report(Error::new(Errc::InvalidArgument, long));

    let mut error = Error::new(Errc::IoError, "root");
    for index in 0..20 {
        error = Error::new(Errc::InvalidState, format!("layer-{index}")).with_source(error);
    }
    diagnostics.report(error);

    let snapshot = diagnostics.snapshot();
    let first = &snapshot.reports()[0];
    assert!(first.summary().len() <= 2 * 1024);
    assert!(!first.summary().chars().any(char::is_control));
    assert!(snapshot.reports()[1].causes_truncated());
}

#[test]
fn concurrent_reports_keep_unique_monotonic_retained_ids() {
    install_global_default_subscriber();
    let diagnostics = Diagnostics::new(
        DiagnosticsConfig::default()
            .report_capacity(16)
            .backtrace(BacktracePolicy::Disabled),
    );
    let mut workers = Vec::new();
    for worker in 0..8 {
        let diagnostics = diagnostics.clone();
        workers.push(std::thread::spawn(move || {
            for item in 0..20 {
                diagnostics.report(Error::new(
                    Errc::PlatformError,
                    format!("worker-{worker}-item-{item}"),
                ));
            }
        }));
    }
    for worker in workers {
        worker
            .join()
            .unwrap_or_else(|_| panic!("report worker must not panic"));
    }

    let snapshot = diagnostics.snapshot();
    assert_eq!(snapshot.total_reports(), 160);
    assert_eq!(snapshot.reports().len(), 16);
    assert_eq!(snapshot.evicted_reports(), 144);
    assert!(
        snapshot
            .reports()
            .windows(2)
            .all(|pair| pair[0].id() < pair[1].id())
    );
}

#[test]
fn exact_error_recovery_is_ordered_raii_scoped_and_panic_safe() {
    install_global_default_subscriber();
    let diagnostics = Diagnostics::default();
    let calls = Arc::new(AtomicUsize::new(0));

    let first_calls = Arc::clone(&calls);
    let first = diagnostics.on_error(Errc::GraphicsDeviceLost, move |_| {
        first_calls.fetch_add(1, Ordering::Relaxed);
        RecoveryAction::NotHandled
    });
    let second_calls = Arc::clone(&calls);
    let second = diagnostics.on_error(Errc::GraphicsDeviceLost, move |_| {
        second_calls.fetch_add(10, Ordering::Relaxed);
        RecoveryAction::Recovered
    });

    assert!(matches!(
        diagnostics.attempt_recovery(Error::new(Errc::GraphicsDeviceLost, "device lost",)),
        RecoveryOutcome::Recovered
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 11);

    drop(second);
    assert!(matches!(
        diagnostics.attempt_recovery(Error::new(Errc::GraphicsDeviceLost, "device lost again",)),
        RecoveryOutcome::Unhandled(_)
    ));
    drop(first);

    let panicking = diagnostics.on_error(Errc::TaskAbandoned, |_| {
        panic!("recovery panic must be isolated")
    });
    let outcome = diagnostics.attempt_recovery(Error::new(Errc::TaskAbandoned, "worker failed"));
    let failure = match outcome {
        RecoveryOutcome::Failed(error) => error,
        _ => panic!("panicking recovery must become a typed failure"),
    };
    assert_eq!(failure.code(), Errc::TaskAbandoned);
    assert!(failure.source_error().is_some());
    drop(panicking);
}

#[derive(Clone)]
struct CapturedEvent {
    target: String,
    level: tracing::Level,
    fields: Vec<String>,
}

struct CaptureSubscriber {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl Subscriber for CaptureSubscriber {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let metadata = event.metadata();
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(CapturedEvent {
                target: metadata.target().to_owned(),
                level: *metadata.level(),
                fields: metadata
                    .fields()
                    .iter()
                    .map(|field| field.name().to_owned())
                    .collect(),
            });
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

#[test]
fn debug_mode_is_runtime_scoped_shared_and_structured() {
    install_global_default_subscriber();
    let events = Arc::new(Mutex::new(Vec::new()));
    let subscriber = CaptureSubscriber {
        events: Arc::clone(&events),
    };
    let diagnostics = Diagnostics::new(DiagnosticsConfig::default().debug_mode(true));
    let shared = diagnostics.clone();

    assert!(diagnostics.debug_mode());
    tracing::subscriber::with_default(subscriber, || {
        diagnostics.set_debug_mode(false);
        // 相同值不应产生重复模式事件。
        diagnostics.set_debug_mode(false);
    });
    assert!(!shared.debug_mode());

    let events = events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].target, "uix::diagnostics");
    for field in ["debug_event", "runtime_id", "enabled"] {
        assert!(events[0].fields.iter().any(|candidate| candidate == field));
    }
}

#[test]
fn report_emits_one_fixed_target_event_with_contract_fields() {
    install_global_default_subscriber();
    let events = Arc::new(Mutex::new(Vec::new()));
    let subscriber = CaptureSubscriber {
        events: Arc::clone(&events),
    };
    let diagnostics = Diagnostics::default();

    tracing::subscriber::with_default(subscriber, || {
        diagnostics.report(Error::new(Errc::PlatformError, "platform failed"));
    });

    let events = events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].target, "uix::diagnostics");
    assert_eq!(events[0].level, tracing::Level::ERROR);
    for field in [
        "uix.report_id",
        "uix.error.code",
        "uix.error.severity",
        "uix.error.summary",
        "uix.runtime_id",
        "uix.origin.target",
    ] {
        assert!(events[0].fields.iter().any(|candidate| candidate == field));
    }
}

struct ReentrantSubscriber {
    diagnostics: Diagnostics,
    entered: AtomicBool,
}

impl Subscriber for ReentrantSubscriber {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, _event: &Event<'_>) {
        if !self.entered.swap(true, Ordering::AcqRel) {
            self.diagnostics
                .report(Error::new(Errc::Unknown, "subscriber reentry"));
        }
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

#[test]
fn reentrant_report_is_retained_but_recursive_event_is_suppressed() {
    install_global_default_subscriber();
    let diagnostics = Diagnostics::default();
    let subscriber = ReentrantSubscriber {
        diagnostics: diagnostics.clone(),
        entered: AtomicBool::new(false),
    };

    tracing::subscriber::with_default(subscriber, || {
        diagnostics.report(Error::new(Errc::IoError, "outer"));
    });

    let snapshot = diagnostics.snapshot();
    assert_eq!(snapshot.reports().len(), 2);
    assert!(snapshot.reports()[0].event_emitted());
    assert!(!snapshot.reports()[1].event_emitted());
}

struct PanicSubscriber;

impl Subscriber for PanicSubscriber {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, _event: &Event<'_>) {
        panic!("subscriber failure")
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

#[test]
fn subscriber_panic_does_not_escape_or_discard_the_report() {
    install_global_default_subscriber();
    let diagnostics = Diagnostics::default();
    tracing::subscriber::with_default(PanicSubscriber, || {
        diagnostics.report(Error::new(Errc::IoError, "retained"));
    });

    let snapshot = diagnostics.snapshot();
    assert_eq!(snapshot.reports().len(), 1);
    assert!(!snapshot.reports()[0].event_emitted());
}

#[test]
fn ordinary_tracing_error_does_not_create_an_error_report() {
    install_global_default_subscriber();
    let diagnostics = Diagnostics::default();
    tracing::error!(target: "application", "ordinary log");
    assert_eq!(diagnostics.snapshot().total_reports(), 0);
}

#[test]
fn report_only_observes_and_recovery_only_recovers() {
    install_global_default_subscriber();
    let diagnostics = Diagnostics::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let handler_calls = Arc::clone(&calls);
    let _subscription = diagnostics.on_error(Errc::GraphicsDeviceLost, move |_| {
        handler_calls.fetch_add(1, Ordering::Relaxed);
        RecoveryAction::Recovered
    });

    // report 只建立观察事实，不得隐式执行已登记的恢复 handler。
    let id = diagnostics.report(Error::new(Errc::GraphicsDeviceLost, "observed only"));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let snapshot = diagnostics.snapshot();
    assert_eq!(snapshot.total_reports(), 1);
    assert_eq!(snapshot.reports()[0].id(), id);

    // attempt_recovery 只尝试恢复，不得隐式产生新的错误报告。
    assert!(matches!(
        diagnostics.attempt_recovery(Error::new(Errc::GraphicsDeviceLost, "recover")),
        RecoveryOutcome::Recovered
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(diagnostics.snapshot().total_reports(), 1);
}

#[test]
fn backtrace_policy_controls_capture_at_the_report_boundary() {
    install_global_default_subscriber();
    let fatal_only = Diagnostics::new(DiagnosticsConfig::default().backtrace(BacktracePolicy::FatalOnly));
    fatal_only.report(Error::fatal(Errc::InvalidState, "fatal captures"));
    fatal_only.report(Error::new(Errc::IoError, "non-fatal does not capture"));

    let errors_and_fatal =
        Diagnostics::new(DiagnosticsConfig::default().backtrace(BacktracePolicy::ErrorsAndFatal));
    errors_and_fatal.report(Error::new(Errc::IoError, "error captures"));

    let disabled =
        Diagnostics::new(DiagnosticsConfig::default().backtrace(BacktracePolicy::Disabled));
    disabled.report(Error::fatal(Errc::InvalidState, "disabled never captures"));

    let fatal_only_snapshot = fatal_only.snapshot();
    assert!(fatal_only_snapshot.reports()[0].backtrace().is_some());
    assert!(fatal_only_snapshot.reports()[1].backtrace().is_none());
    assert!(errors_and_fatal.snapshot().reports()[0].backtrace().is_some());
    assert!(disabled.snapshot().reports()[0].backtrace().is_none());
}

const REBUILD_TEST_TARGET: &str = "uix::diagnostics-rebuild-test";

#[test]
fn rebuild_tracing_interest_cache_keeps_callsites_delivering() {
    install_global_default_subscriber();
    let events = Arc::new(Mutex::new(Vec::new()));
    let subscriber = CaptureSubscriber {
        events: Arc::clone(&events),
    };
    let diagnostics = Diagnostics::default();

    tracing::subscriber::with_default(subscriber, || {
        // 重建前事件正常送达。
        tracing::event!(target: REBUILD_TEST_TARGET, tracing::Level::INFO, "before rebuild");
        assert_eq!(
            events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .filter(|event| event.target == REBUILD_TEST_TARGET)
                .count(),
            1
        );

        // 重建 interest 缓存后，同一 callsite 对当前 dispatch 保持可见。
        diagnostics.rebuild_tracing_interest_cache();
        tracing::event!(target: REBUILD_TEST_TARGET, tracing::Level::INFO, "after rebuild");
        assert_eq!(
            events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .filter(|event| event.target == REBUILD_TEST_TARGET)
                .count(),
            2,
            "rebuild must keep already-registered callsites delivering"
        );
    });
}

/// 瞬态观察去重：同一 (target, reason) 首条发射、冷却窗口内重复抑制，
/// 不同键互不影响。返回值让去重行为无需等待真实冷却时间即可锁定。
#[test]
fn observe_transient_deduplicates_within_cooldown_window() {
    let diagnostics = Diagnostics::new(DiagnosticsConfig::default());

    // 首条立即发射。
    assert!(
        diagnostics.observe_transient("test_layer", "first failure", "detail-a"),
        "first observation of a key must emit"
    );
    // 冷却窗口内的重复观察被抑制。
    assert!(
        !diagnostics.observe_transient("test_layer", "first failure", "detail-b"),
        "repeated observation within the cooldown must be suppressed"
    );
    assert!(
        !diagnostics.observe_transient("test_layer", "first failure", "detail-c"),
        "further repeats stay suppressed within the same window"
    );
    // 不同 reason 是独立键，不受首个键的窗口影响。
    assert!(
        diagnostics.observe_transient("test_layer", "second failure", "detail-d"),
        "a different reason is an independent key and must emit"
    );
    // 不同 target 同样独立。
    assert!(
        diagnostics.observe_transient("other_layer", "first failure", "detail-e"),
        "a different target is an independent key and must emit"
    );
    // 抑制不影响报告面：瞬态观察不产生任何留存报告。
    let snapshot = diagnostics.snapshot();
    assert_eq!(
        snapshot.total_reports(),
        0,
        "transient observations must not enter the retained report store"
    );
}
