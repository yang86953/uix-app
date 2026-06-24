// ============================================================================
// uix-platform/src/log.rs — Core logging: Level, Logger, and free functions
// ============================================================================

use crate::error::Error;
use chrono::Local;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

// Re-export items from log_format so api.rs and collector.rs find them here.
pub use crate::log_format::{CallbackSink, ConsoleSink, FileSink, Record, Sink};

// ════════════════════════════════════════════════════════════════════════════
// 日志级别
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Level {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Fatal = 5,
}

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Trace => "TRACE",
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
            Level::Fatal => "FATAL",
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Logger — 单例日志器
// ════════════════════════════════════════════════════════════════════════════

struct LoggerInner {
    sinks: Vec<Arc<dyn Sink>>,
}

pub struct Logger {
    inner: RwLock<LoggerInner>,
    level: RwLock<Level>,
    sequence: AtomicU64,
}

impl Logger {
    pub fn instance() -> &'static Self {
        static LOGGER: std::sync::OnceLock<Logger> = std::sync::OnceLock::new();
        LOGGER.get_or_init(|| {
            let logger = Logger {
                inner: RwLock::new(LoggerInner { sinks: Vec::new() }),
                level: RwLock::new(Level::Warn),
                sequence: AtomicU64::new(0),
            };
            // Default sink: console
            logger.add_sink(Arc::new(ConsoleSink::new(true)));
            logger
        })
    }

    pub fn add_sink(&self, sink: Arc<dyn Sink>) {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .sinks
            .push(sink);
    }

    pub fn remove_sink(&self, sink_ptr: *const dyn Sink) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner
            .sinks
            .retain(|s| !std::ptr::addr_eq(Arc::as_ptr(s), sink_ptr));
    }

    pub fn clear_sinks(&self) {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .sinks
            .clear();
    }

    pub fn set_level(&self, level: Level) {
        *self.level.write().unwrap_or_else(|e| e.into_inner()) = level;
    }

    pub fn get_level(&self) -> Level {
        *self.level.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn flush(&self) {
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        for sink in &inner.sinks {
            sink.flush();
        }
    }

    pub fn log(
        &self,
        level: Level,
        message: String,
        file: &'static str,
        line: u32,
        attributes: Vec<(String, String)>,
    ) {
        if (level as u8) < (self.get_level() as u8) {
            return;
        }
        let record = Record {
            level,
            timestamp: Local::now(),
            file,
            line,
            message,
            attributes,
            sequence: self.sequence.fetch_add(1, Ordering::Relaxed),
        };

        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        for sink in &inner.sinks {
            if sink.passes(level) {
                sink.write(&record);
            }
        }
    }

    pub fn trace(&self, msg: String, file: &'static str, line: u32) {
        self.log(Level::Trace, msg, file, line, Vec::new());
    }

    pub fn debug(&self, msg: String, file: &'static str, line: u32) {
        self.log(Level::Debug, msg, file, line, Vec::new());
    }

    pub fn info(&self, msg: String, file: &'static str, line: u32) {
        self.log(Level::Info, msg, file, line, Vec::new());
    }

    pub fn warn(&self, msg: String, file: &'static str, line: u32) {
        self.log(Level::Warn, msg, file, line, Vec::new());
    }

    pub fn error(&self, msg: String, file: &'static str, line: u32) {
        self.log(Level::Error, msg, file, line, Vec::new());
    }

    pub fn fatal(&self, msg: String, file: &'static str, line: u32) {
        self.log(Level::Fatal, msg, file, line, Vec::new());
    }

    pub fn log_error(&self, error: &Error, level: Level) {
        self.log(
            level,
            error.short_what(),
            error.file(),
            error.line(),
            Vec::new(),
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 自由函数 — 使用 #[track_caller] 捕获调用者位置
// ════════════════════════════════════════════════════════════════════════════

/// 记录一条 TRACE 级别日志，自动捕获调用者源位置。
#[track_caller]
pub fn trace_fn(msg: impl fmt::Display) {
    let loc = std::panic::Location::caller();
    Logger::instance().log(
        Level::Trace,
        msg.to_string(),
        loc.file(),
        loc.line(),
        Vec::new(),
    );
}

/// 记录一条 DEBUG 级别日志，自动捕获调用者源位置。
#[track_caller]
pub fn debug_fn(msg: impl fmt::Display) {
    let loc = std::panic::Location::caller();
    Logger::instance().log(
        Level::Debug,
        msg.to_string(),
        loc.file(),
        loc.line(),
        Vec::new(),
    );
}

/// 记录一条 INFO 级别日志，自动捕获调用者源位置。
#[track_caller]
pub fn info_fn(msg: impl fmt::Display) {
    let loc = std::panic::Location::caller();
    Logger::instance().log(
        Level::Info,
        msg.to_string(),
        loc.file(),
        loc.line(),
        Vec::new(),
    );
}

/// 记录一条 WARN 级别日志，自动捕获调用者源位置。
#[track_caller]
pub fn warn_fn(msg: impl fmt::Display) {
    let loc = std::panic::Location::caller();
    Logger::instance().log(
        Level::Warn,
        msg.to_string(),
        loc.file(),
        loc.line(),
        Vec::new(),
    );
}

/// 记录一条 ERROR 级别日志，自动捕获调用者源位置。
#[track_caller]
pub fn error_fn(msg: impl fmt::Display) {
    let loc = std::panic::Location::caller();
    Logger::instance().log(
        Level::Error,
        msg.to_string(),
        loc.file(),
        loc.line(),
        Vec::new(),
    );
}

/// 记录一条 FATAL 级别日志，自动捕获调用者源位置。
#[track_caller]
pub fn fatal_fn(msg: impl fmt::Display) {
    let loc = std::panic::Location::caller();
    Logger::instance().log(
        Level::Fatal,
        msg.to_string(),
        loc.file(),
        loc.line(),
        Vec::new(),
    );
}

pub fn log_error(error: &Error) {
    Logger::instance().log_error(error, Level::Error);
}
