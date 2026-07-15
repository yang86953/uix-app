use crate::core::log::logger::*;
use crate::core::log::{CallbackSink, HandlerSlot, LogHandler, Record, Sink};
use crate::core::{Errc, Error};
use crate::tests::common::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

struct ReentrantSink {
    logger: Weak<Logger>,
    writes: AtomicUsize,
    flushes: AtomicUsize,
}

struct CountingHandler {
    calls: Arc<AtomicUsize>,
}

struct PanickingSink;

#[derive(Default)]
struct CountingSink {
    writes: AtomicUsize,
    flushes: AtomicUsize,
}

impl LogHandler for CountingHandler {
    fn handle(&self, _level: Level, _message: &str, _file: &'static str, _line: u32) {
        self.calls.fetch_add(1, Ordering::Relaxed);
    }
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

impl Sink for PanickingSink {
    fn write(&self, _record: &Record) {
        panic!("intentional sink write panic");
    }

    fn flush(&self) {
        panic!("intentional sink flush panic");
    }

    fn set_level(&self, _level: Level) {}

    fn level(&self) -> Level {
        Level::Trace
    }
}

impl Sink for CountingSink {
    fn write(&self, _record: &Record) {
        self.writes.fetch_add(1, Ordering::Relaxed);
    }

    fn flush(&self) {
        self.flushes.fetch_add(1, Ordering::Relaxed);
    }

    fn set_level(&self, _level: Level) {}

    fn level(&self) -> Level {
        Level::Trace
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
fn panicking_sink_does_not_interrupt_later_write_or_flush() {
    let logger = Logger::with_default_console();
    logger.clear_sinks();
    logger.set_level(Level::Trace);
    logger.add_sink(Arc::new(PanickingSink));
    let counting = Arc::new(CountingSink::default());
    logger.add_sink(counting.clone());

    logger.info("probe".to_owned(), file!(), line!());
    logger.flush();

    assert_eq!(counting.writes.load(Ordering::Relaxed), 1);
    assert_eq!(counting.flushes.load(Ordering::Relaxed), 1);
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

#[test]
fn default_handler_access_does_not_block_the_first_explicit_registration() {
    let mut slot = HandlerSlot::with_fallback();
    let _fallback = slot.handler();
    let first_calls = Arc::new(AtomicUsize::new(0));
    let second_calls = Arc::new(AtomicUsize::new(0));

    assert!(slot.install(Box::new(CountingHandler {
        calls: Arc::clone(&first_calls),
    })));
    assert!(!slot.install(Box::new(CountingHandler {
        calls: Arc::clone(&second_calls),
    })));
    slot.handler().handle(Level::Info, "probe", "handler.rs", 1);

    assert_eq!(first_calls.load(Ordering::Relaxed), 1);
    assert_eq!(second_calls.load(Ordering::Relaxed), 0);
}
