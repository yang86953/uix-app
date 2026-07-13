use crate::tests::common::*;
use std::sync::atomic::{ AtomicU64 };
use std::sync::{ RwLock };
use crate::core::diagnostic::Timestamp;
use crate::core::log::logger::*;

#[test]
fn default_console_obeys_the_global_logger_level() {
    let logger = Logger::with_default_console();
    let inner = logger.inner.read().unwrap_or_else(|e| e.into_inner());

    assert_eq!(logger.get_level(), Level::Warn);
    assert_eq!(inner.sinks.len(), 1);
    assert_eq!(inner.sinks[0].level(), Level::Trace);
}
