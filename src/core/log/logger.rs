// ============================================================================
// platform/src/log/logger.rs — Logger 单例（高级日志）
//
// 提供结构化日志记录，支持多 Sink 输出（控制台、文件、回调）。
// 首次初始化时自动注册为 platform 全局日志处理器。
// ============================================================================

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use super::sink::{Record, Sink};
use super::Level;
use super::LogHandler;
use crate::core::diagnostic::Timestamp;
use crate::native::Error;

/// 将 Logger 桥接到 platform LogHandler trait。
struct LoggerHandler;

impl LogHandler for LoggerHandler {
    fn handle(&self, level: Level, message: &str, file: &'static str, line: u32) {
        Logger::instance().log(level, message.to_string(), file, line, Vec::new());
    }

    fn set_level(&self, level: Level) {
        Logger::instance().set_level(level);
    }

    fn level(&self) -> Level {
        Logger::instance().get_level()
    }

    fn flush(&self) {
        Logger::instance().flush();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Logger 单例
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
            // 默认输出：控制台
            logger.add_sink(Arc::new(super::sink::ConsoleSink::new(true)));
            // 注册为 platform 全局日志处理器
            crate::core::log::set_handler(Box::new(LoggerHandler));
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
            timestamp: Timestamp::now(),
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
