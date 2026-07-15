use crate::core::diagnostic::{Collector, CollectorConfig};
use crate::core::log::Level;
use crate::core::{Errc, Error, ErrorSeverity};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[test]
fn configure_immediately_enforces_the_new_capacity() {
    let collector = Collector::for_test(CollectorConfig {
        max_errors: 3,
        auto_log: false,
        ..CollectorConfig::default()
    });
    collector.collect(Error::warn(Errc::Unknown, "first"));
    collector.collect(Error::warn(Errc::Unknown, "second"));
    collector.collect(Error::warn(Errc::Unknown, "third"));

    collector.configure(CollectorConfig {
        max_errors: 1,
        auto_log: false,
        ..CollectorConfig::default()
    });

    assert_eq!(collector.stored_count(), 1);
    assert_eq!(collector.errors()[0].message(), "third");
}

#[test]
fn collector_auto_log_policy_honors_enablement_and_configured_level() {
    let collector = Collector::for_test(CollectorConfig {
        auto_log: true,
        auto_log_level: Level::Debug,
        ..CollectorConfig::default()
    });

    assert_eq!(
        collector.configured_log_level_for_test(ErrorSeverity::Warning),
        Some(Level::Debug)
    );

    collector.set_auto_log(false, Level::Trace);
    assert_eq!(
        collector.configured_log_level_for_test(ErrorSeverity::Error),
        None
    );
    assert_eq!(
        collector.configured_log_level_for_test(ErrorSeverity::Fatal),
        Some(Level::Fatal)
    );
}

#[test]
fn collector_callback_can_reenter_without_holding_internal_lock() {
    const MESSAGE: &str = "collector reentrant callback probe";

    let observed = Arc::new(AtomicBool::new(false));
    let callback_observed = Arc::clone(&observed);
    let collector = Collector::instance();
    let token = collector.on_collect(move |error| {
        if error.message() == MESSAGE {
            let _ = Collector::instance().stored_count();
            callback_observed.store(true, Ordering::Release);
        }
    });

    collector.collect(Error::new(Errc::Unknown, MESSAGE));

    assert!(observed.load(Ordering::Acquire));
    assert!(collector.remove_callback(token));
}
