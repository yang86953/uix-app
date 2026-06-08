// ============================================================================
// uix-platform/src/log.rs — Structured logging with levels, sinks, and formatting
// ============================================================================

use crate::diag::error::Error;
use chrono::{DateTime, Local};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

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
// 日志记录
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct Record {
    pub level: Level,
    pub timestamp: DateTime<Local>,
    pub file: &'static str,
    pub line: u32,
    pub message: String,
    pub attributes: Vec<(String, String)>,
    pub sequence: u64,
}

impl Record {
    pub fn to_string(&self) -> String {
        let ts = self.timestamp.format("%H:%M:%S%.3f");
        let filename = Path::new(self.file)
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_else(|| std::borrow::Cow::Borrowed(self.file));

        let mut result = format!(
            "{} [{:7}] {} ({}:{})",
            ts,
            self.level.name(),
            self.message,
            filename,
            self.line
        );
        if !self.attributes.is_empty() {
            result.push_str(" {");
            for (i, (k, v)) in self.attributes.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&format!("{}={}", k, v));
            }
            result.push('}');
        }
        result
    }

    pub fn to_json(&self) -> String {
        let ts = self.timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ");
        let filename = Path::new(self.file)
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_else(|| std::borrow::Cow::Borrowed(self.file));

        let mut json = format!(
            r#"{{"ts":"{}","level":"{}","msg":"{}","file":"{}","line":{}}}"#,
            ts,
            self.level.name(),
            self.message,
            filename,
            self.line
        );
        if !self.attributes.is_empty() {
            json = json.trim_end_matches('}').to_string();
            json.push_str(r#","attrs":{)"#);
            for (i, (k, v)) in self.attributes.iter().enumerate() {
                if i > 0 {
                    json.push(',');
                }
                json.push_str(&format!(r#""{}":"{}""#, k, v));
            }
            json.push_str("}}");
        }
        json
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Sink trait
// ════════════════════════════════════════════════════════════════════════════

pub trait Sink: Send + Sync {
    fn write(&self, record: &Record);
    fn flush(&self) {}
    fn set_level(&self, level: Level);
    fn level(&self) -> Level;
    fn passes(&self, level: Level) -> bool {
        level as u8 >= self.level() as u8
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ConsoleSink
// ════════════════════════════════════════════════════════════════════════════

pub struct ConsoleSink {
    level: RwLock<Level>,
}

impl ConsoleSink {
    pub fn new(_use_color: bool) -> Self {
        Self {
            level: RwLock::new(Level::Trace),
        }
    }
}

impl Sink for ConsoleSink {
    fn write(&self, record: &Record) {
        let line = format!("{}\n", record.to_string());
        if record.level >= Level::Warn {
            let _ = io::Write::write(&mut io::stderr(), line.as_bytes());
        } else {
            let _ = io::Write::write(&mut io::stdout(), line.as_bytes());
        }
    }

    fn flush(&self) {
        let _ = io::stdout().flush();
        let _ = io::stderr().flush();
    }

    fn set_level(&self, level: Level) {
        *self.level.write().unwrap_or_else(|e| e.into_inner()) = level;
    }

    fn level(&self) -> Level {
        *self.level.read().unwrap_or_else(|e| e.into_inner())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// FileSink
// ════════════════════════════════════════════════════════════════════════════

pub struct FileSink {
    path: String,
    level: RwLock<Level>,
    max_size: u64,
    file: Mutex<File>,
}

impl FileSink {
    pub fn new(path: impl Into<String>) -> io::Result<Self> {
        let path = path.into();
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            path,
            level: RwLock::new(Level::Trace),
            max_size: 0,
            file: Mutex::new(file),
        })
    }

    pub fn set_max_size(&mut self, bytes: u64) {
        self.max_size = bytes;
    }

    fn rotate(&self) {
        if self.max_size == 0 {
            return;
        }
        if let Ok(metadata) = fs::metadata(&self.path) {
            if metadata.len() < self.max_size {
                return;
            }
        } else {
            return;
        }

        // Rotate files .1, .2, ... .9
        for i in (0..9).rev() {
            let old = if i == 0 {
                self.path.clone()
            } else {
                format!("{}.{}", self.path, i)
            };
            let new = format!("{}.{}", self.path, i + 1);
            if Path::new(&old).exists() {
                if i == 9 {
                    let _ = fs::remove_file(&old);
                } else {
                    let _ = fs::rename(&old, &new);
                }
            }
        }

        // Reopen file
        let file = match OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)
        {
            Ok(f) => f,
            Err(e) => {
                eprintln!("failed to reopen log file after rotation: {}", e);
                return;
            }
        };
        *self.file.lock().unwrap_or_else(|e| e.into_inner()) = file;
    }
}

impl Sink for FileSink {
    fn write(&self, record: &Record) {
        let line = format!("{}\n", record.to_json());
        let mut file = self.file.lock().unwrap_or_else(|e| e.into_inner());
        let _ = file.write_all(line.as_bytes());
        self.rotate();
    }

    fn flush(&self) {
        let _ = self.file.lock().unwrap_or_else(|e| e.into_inner()).flush();
    }

    fn set_level(&self, level: Level) {
        *self.level.write().unwrap_or_else(|e| e.into_inner()) = level;
    }

    fn level(&self) -> Level {
        *self.level.read().unwrap_or_else(|e| e.into_inner())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// CallbackSink
// ════════════════════════════════════════════════════════════════════════════

pub struct CallbackSink {
    level: RwLock<Level>,
    callback: Box<dyn Fn(&Record) + Send + Sync>,
}

impl CallbackSink {
    pub fn new(callback: impl Fn(&Record) + Send + Sync + 'static) -> Self {
        Self {
            level: RwLock::new(Level::Trace),
            callback: Box::new(callback),
        }
    }
}

impl Sink for CallbackSink {
    fn write(&self, record: &Record) {
        (self.callback)(record);
    }

    fn set_level(&self, level: Level) {
        *self.level.write().unwrap_or_else(|e| e.into_inner()) = level;
    }

    fn level(&self) -> Level {
        *self.level.read().unwrap_or_else(|e| e.into_inner())
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
                level: RwLock::new(Level::Trace),
                sequence: AtomicU64::new(0),
            };
            // Default sink: console
            logger.add_sink(Arc::new(ConsoleSink::new(true)));
            logger
        })
    }

    pub fn add_sink(&self, sink: Arc<dyn Sink>) {
        self.inner.write().unwrap_or_else(|e| e.into_inner()).sinks.push(sink);
    }

    pub fn remove_sink(&self, sink_ptr: *const dyn Sink) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.sinks.retain(|s| !std::ptr::addr_eq(Arc::as_ptr(s), sink_ptr));
    }

    pub fn clear_sinks(&self) {
        self.inner.write().unwrap_or_else(|e| e.into_inner()).sinks.clear();
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
// 自由函数
// ════════════════════════════════════════════════════════════════════════════

macro_rules! log_fn {
    ($name:ident, $level:expr) => {
        pub fn $name(msg: impl fmt::Display) {
            Logger::instance().log($level, msg.to_string(), file!(), line!(), Vec::new());
        }
    };
}

log_fn!(trace_fn, Level::Trace);
log_fn!(debug_fn, Level::Debug);
log_fn!(info_fn, Level::Info);
log_fn!(warn_fn, Level::Warn);
log_fn!(error_fn, Level::Error);
log_fn!(fatal_fn, Level::Fatal);

pub fn log_error(error: &Error) {
    Logger::instance().log_error(error, Level::Error);
}
