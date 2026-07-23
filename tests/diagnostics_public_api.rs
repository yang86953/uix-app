use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};
use uix::core::{Errc, Error, ErrorSeverity};
use uix::diagnostics::{
    BacktracePolicy, Diagnostics, DiagnosticsConfig, RecoveryAction, RecoveryOutcome,
};

#[test]
fn report_is_retained_without_a_subscriber_and_snapshot_is_immutable() {
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
    let diagnostics = Diagnostics::new(
        DiagnosticsConfig::default().backtrace(BacktracePolicy::Disabled),
    );
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
        worker.join().expect("report worker must not panic");
    }

    let snapshot = diagnostics.snapshot();
    assert_eq!(snapshot.total_reports(), 160);
    assert_eq!(snapshot.reports().len(), 16);
    assert_eq!(snapshot.evicted_reports(), 144);
    assert!(snapshot
        .reports()
        .windows(2)
        .all(|pair| pair[0].id() < pair[1].id()));
}

#[test]
fn exact_error_recovery_is_ordered_raii_scoped_and_panic_safe() {
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
        diagnostics.attempt_recovery(Error::new(
            Errc::GraphicsDeviceLost,
            "device lost",
        )),
        RecoveryOutcome::Recovered
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 11);

    drop(second);
    assert!(matches!(
        diagnostics.attempt_recovery(Error::new(
            Errc::GraphicsDeviceLost,
            "device lost again",
        )),
        RecoveryOutcome::Unhandled(_)
    ));
    drop(first);

    let panicking = diagnostics.on_error(Errc::TaskAbandoned, |_| {
        panic!("recovery panic must be isolated")
    });
    let outcome =
        diagnostics.attempt_recovery(Error::new(Errc::TaskAbandoned, "worker failed"));
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
fn report_emits_one_fixed_target_event_with_contract_fields() {
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
    let diagnostics = Diagnostics::default();
    tracing::error!(target: "application", "ordinary log");
    assert_eq!(diagnostics.snapshot().total_reports(), 0);
}
