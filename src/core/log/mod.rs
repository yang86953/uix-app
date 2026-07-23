// ============================================================================
// core/log/mod.rs — 核心日志基础设施
//
// 所有 crate 通过本模块的 info_fn / warn_fn / error_fn 等自由函数输出日志，
// 不再依赖外部 `log` crate。
//
// 子模块：
//   sink.rs   — Record, Sink trait, ConsoleSink, FileSink, CallbackSink
//   logger.rs — Logger 单例（高级日志，自动注册为全局处理器）
// ============================================================================

pub mod logger;
pub mod sink;

use std::fmt;
use std::sync::{Arc, OnceLock, RwLock};

use crate::core::diagnostic::Collector;
use crate::core::error::{Errc, Error};

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
// 日志处理器 trait — 允许 services 注册高级 Logger
// ════════════════════════════════════════════════════════════════════════════

pub trait LogHandler: Send + Sync {
    /// 处理一条日志记录。
    /// `file` 来自 `#[track_caller]`，始终为 `'static`。
    fn handle(&self, level: Level, message: &str, file: &'static str, line: u32);

    /// 设置日志级别过滤器（可选覆盖）。
    fn set_level(&self, _level: Level) {}

    /// 获取当前日志级别。
    fn level(&self) -> Level {
        Level::Trace
    }

    /// 刷新所有待写入的日志。
    fn flush(&self) {}
}

// ── 极简时间格式化（HH:MM:SS.fff）────────────────────────────────────────

/// 返回当前 UTC 时间的 `HH:MM:SS.fff` 格式字符串。
fn now_pretty() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_ms = d.as_millis();
    let total_secs = (total_ms / 1000) as u64;
    let ms = (total_ms % 1000) as u32;
    let h = (total_secs / 3600) % 24;
    let m = (total_secs / 60) % 60;
    let s = total_secs % 60;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

// ════════════════════════════════════════════════════════════════════════════
// 默认处理器 — 输出到 stderr
// ════════════════════════════════════════════════════════════════════════════

struct DefaultHandler {
    level: Level,
}

impl DefaultHandler {
    const fn new(level: Level) -> Self {
        Self { level }
    }
}

impl LogHandler for DefaultHandler {
    fn handle(&self, level: Level, message: &str, file: &'static str, line: u32) {
        if (level as u8) < (self.level as u8) {
            return;
        }
        let filename = file
            .rsplit_once('/')
            .or_else(|| file.rsplit_once('\\'))
            .map(|(_, name)| name)
            .unwrap_or(file);
        let ts = now_pretty();
        eprintln!("{ts} [{level}] {message} ({filename}:{line})");
    }

    fn set_level(&self, _level: Level) {}

    fn level(&self) -> Level {
        self.level
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 全局处理器
// ════════════════════════════════════════════════════════════════════════════

pub(crate) struct HandlerSlot {
    handler: Arc<dyn LogHandler>,
    is_fallback: bool,
}

impl HandlerSlot {
    pub(crate) fn with_fallback() -> Self {
        Self {
            handler: Arc::new(DefaultHandler::new(Level::Warn)),
            is_fallback: true,
        }
    }

    pub(crate) fn install(&mut self, handler: Box<dyn LogHandler>) -> bool {
        if !self.is_fallback {
            return false;
        }
        self.handler = handler.into();
        self.is_fallback = false;
        true
    }

    pub(crate) fn handler(&self) -> Arc<dyn LogHandler> {
        Arc::clone(&self.handler)
    }
}

static GLOBAL_HANDLER: OnceLock<RwLock<HandlerSlot>> = OnceLock::new();

fn handler_slot() -> &'static RwLock<HandlerSlot> {
    GLOBAL_HANDLER.get_or_init(|| RwLock::new(HandlerSlot::with_fallback()))
}

/// 注册首个显式全局日志处理器。提前使用默认 fallback 不占用注册名额。
pub fn set_handler(handler: Box<dyn LogHandler>) {
    let _ = handler_slot()
        .write()
        .unwrap_or_else(|error| error.into_inner())
        .install(handler);
}

/// 获取当前日志处理器。默认使用 DefaultHandler（stderr 输出）。
fn handler() -> Arc<dyn LogHandler> {
    handler_slot()
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .handler()
}

/// 设置日志级别（委托给当前处理器）。
pub fn set_level(level: Level) {
    handler().set_level(level);
}

/// 获取当前日志级别。
pub fn level() -> Level {
    handler().level()
}

/// 刷新所有日志输出。
pub fn flush() {
    handler().flush();
}

// ════════════════════════════════════════════════════════════════════════════
// 自由函数 — 使用 #[track_caller] 自动捕获调用者位置
// ════════════════════════════════════════════════════════════════════════════

pub(crate) fn render_message_if_enabled(
    handler: &dyn LogHandler,
    level: Level,
    msg: impl fmt::Display,
) -> Option<String> {
    (level >= handler.level()).then(|| msg.to_string())
}

fn emit(level: Level, msg: impl fmt::Display, loc: &'static std::panic::Location<'static>) {
    let handler = handler();
    let Some(message) = render_message_if_enabled(handler.as_ref(), level, msg) else {
        return;
    };
    handler.handle(level, &message, loc.file(), loc.line());
}

#[track_caller]
pub fn trace_fn(msg: impl fmt::Display) {
    emit(Level::Trace, msg, std::panic::Location::caller());
}

#[track_caller]
pub fn debug_fn(msg: impl fmt::Display) {
    emit(Level::Debug, msg, std::panic::Location::caller());
}

#[track_caller]
pub fn info_fn(msg: impl fmt::Display) {
    emit(Level::Info, msg, std::panic::Location::caller());
}

#[track_caller]
pub fn warn_fn(msg: impl fmt::Display) {
    emit(Level::Warn, msg, std::panic::Location::caller());
}

#[track_caller]
pub fn error_fn(msg: impl fmt::Display) {
    let location = std::panic::Location::caller();
    let handler = handler();
    let Some(message) = render_message_if_enabled(handler.as_ref(), Level::Error, msg) else {
        return;
    };
    handler.handle(Level::Error, &message, location.file(), location.line());
    Collector::instance().collect_logged(Error::with_location(
        Errc::Unknown,
        message,
        location.file(),
        location.line(),
    ));
}

#[track_caller]
pub fn fatal_fn(msg: impl fmt::Display) {
    emit(Level::Fatal, msg, std::panic::Location::caller());
}

pub(crate) fn format_error_message(error: &Error) -> String {
    format!("{}: {}", error.code(), error.message())
}

/// 记录一个 Error 对象；消息只含 typed code 与正文，级别和位置由日志记录承载。
pub fn log_error(error: &Error, level: Level) {
    handler().handle(
        level,
        &format_error_message(error),
        error.file(),
        error.line(),
    );
}

// ── 便利重导出 ─────────────────────────────────────────────────
pub use logger::Logger;
pub use sink::{CallbackSink, ConsoleSink, FileSink, Record, Sink};
