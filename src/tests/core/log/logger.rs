use crate::core::log::logger::*;
use crate::core::log::{CallbackSink, Record, Sink};
use crate::core::{Errc, Error};
use crate::tests::common::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

struct ReentrantSink {
    logger: Weak<Logger>,
    writes: AtomicUsize,
    flushes: AtomicUsize,
}

impl ReentrantSink {
    fn new(logger: &Arc<Logger>) -> Self {
        Self {
            logger: Arc::downgrade(logger),
            writes: AtomicUsize::new(0),
            flushes: AtomicUsize::new(0),
        }
    }
}

impl Sink for ReentrantSink {
    fn write(&self, _record: &Record) {
        self.writes.fetch_add(1, Ordering::Relaxed);
        if let Some(logger) = self.logger.upgrade() {
            logger.clear_sinks();
        }
    }

    fn flush(&self) {
        self.flushes.fetch_add(1, Ordering::Relaxed);
        if let Some(logger) = self.logger.upgrade() {
            logger.clear_sinks();
        }
    }

    fn set_level(&self, _level: Level) {}

    fn level(&self) -> Level {
        Level::Trace
    }
}

#[test]
fn default_console_obeys_the_global_logger_level() {
    let logger = Logger::with_default_console();
    let inner = logger.inner.read().unwrap_or_else(|e| e.into_inner());

    assert_eq!(logger.get_level(), Level::Warn);
    assert_eq!(inner.sinks.len(), 1);
    assert_eq!(inner.sinks[0].level(), Level::Trace);
}

#[test]
fn sink_write_can_reenter_logger_without_holding_the_sink_lock() {
    let logger = Arc::new(Logger::with_default_console());
    logger.clear_sinks();
    logger.set_level(Level::Trace);
    let sink = Arc::new(ReentrantSink::new(&logger));
    logger.add_sink(sink.clone());

    logger.info("probe".to_owned(), file!(), line!());

    assert_eq!(sink.writes.load(Ordering::Relaxed), 1);
    assert!(logger
        .inner
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .sinks
        .is_empty());
}

#[test]
fn sink_flush_can_reenter_logger_without_holding_the_sink_lock() {
    let logger = Arc::new(Logger::with_default_console());
    logger.clear_sinks();
    let sink = Arc::new(ReentrantSink::new(&logger));
    logger.add_sink(sink.clone());

    logger.flush();

    assert_eq!(sink.flushes.load(Ordering::Relaxed), 1);
    assert!(logger
        .inner
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .sinks
        .is_empty());
}

#[test]
fn error_logging_does_not_duplicate_record_level_or_location() {
    let logger = Logger::with_default_console();
    logger.clear_sinks();
    logger.set_level(Level::Trace);
    let records = Arc::new(Mutex::new(Vec::new()));
    let observed_records = Arc::clone(&records);
    logger.add_sink(Arc::new(CallbackSink::new(move |record| {
        observed_records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(record.clone());
    })));
    let error = Error::with_location(Errc::InvalidState, "bad state", "failure.rs", 17);

    logger.log_error(&error, Level::Error);

    let records = records.lock().unwrap_or_else(|error| error.into_inner());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].message, "invalid_state: bad state");
    let rendered = records[0].to_string();
    assert_eq!(rendered.matches("[ERROR]").count(), 1);
    assert_eq!(rendered.matches("(failure.rs:17)").count(), 1);
}
