// 框架级 Error 结构体

use std::fmt;
use std::panic::Location;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

use super::codes::Errc;
use super::severity::ErrorSeverity;
use super::unhandled;

/// 框架级错误类型。
///
/// 所有层统一使用此类型传递错误。携带错误码、严重级别、消息文本、
/// 源码位置（file/line）、时间戳和可选的上游原因链。
///
/// # 构造示例
///
#[must_use = "框架错误必须经诊断系统处置（report / observe / enqueue / recovery），丢弃会留下未处置观测记录"]
pub struct Error {
    code: Errc,
    message: String,
    severity: ErrorSeverity,
    file: &'static str,
    line: u32,
    timestamp: SystemTime,
    source: Option<Box<Error>>,
    // 诊断处置标记：任一诊断通道消费（或随原因链被父错误承担）后置位，
    // Drop 时仍未置位的错误进入全局未处置观测（有界去重）。
    disposition: AtomicBool,
}

// Clone 不复制处置标记：副本是独立的未处置错误实例，拥有自己的观测责任。
impl Clone for Error {
    fn clone(&self) -> Self {
        Self {
            code: self.code,
            message: self.message.clone(),
            severity: self.severity,
            file: self.file,
            line: self.line,
            timestamp: self.timestamp,
            source: self.source.clone(),
            disposition: AtomicBool::new(false),
        }
    }
}

impl Drop for Error {
    fn drop(&mut self) {
        if !self.disposition.load(Ordering::Acquire) {
            unhandled::record_disposed_without_disposition(self);
        }
    }
}

impl Error {
    // ── 构造器 ──

    /// 创建一个默认严重度为 Error 的框架错误。
    #[track_caller]
    pub fn new(code: Errc, message: impl Into<String>) -> Self {
        let location = Location::caller();
        Self {
            code,
            message: message.into(),
            severity: ErrorSeverity::Error,
            file: location.file(),
            line: location.line(),
            timestamp: SystemTime::now(),
            source: None,
            disposition: AtomicBool::new(false),
        }
    }

    /// 创建一个指定严重度的错误。
    #[track_caller]
    pub fn with_severity(code: Errc, message: impl Into<String>, severity: ErrorSeverity) -> Self {
        let location = Location::caller();
        Self {
            code,
            message: message.into(),
            severity,
            file: location.file(),
            line: location.line(),
            timestamp: SystemTime::now(),
            source: None,
            disposition: AtomicBool::new(false),
        }
    }

    /// 创建一个 Warning 级别的错误。
    #[track_caller]
    pub fn warn(code: Errc, message: impl Into<String>) -> Self {
        Self::with_severity(code, message, ErrorSeverity::Warning)
    }

    /// 创建一个 Info 级别的错误。
    #[track_caller]
    pub fn info(code: Errc, message: impl Into<String>) -> Self {
        Self::with_severity(code, message, ErrorSeverity::Info)
    }

    /// 创建一个 Fatal 级别的错误。
    #[track_caller]
    pub fn fatal(code: Errc, message: impl Into<String>) -> Self {
        Self::with_severity(code, message, ErrorSeverity::Fatal)
    }

    /// 创建一个带有自定义源码位置信息的错误。
    pub fn with_location(
        code: Errc,
        message: impl Into<String>,
        file: &'static str,
        line: u32,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            severity: ErrorSeverity::Error,
            file,
            line,
            timestamp: SystemTime::now(),
            source: None,
            disposition: AtomicBool::new(false),
        }
    }

    // ── 原因链 ──

    /// 为此错误附加一个上游原因。
    ///
    /// 错误链作为一个观测单位：父错误的诊断处置覆盖全链，因此源错误在
    /// 附加时即视为随链处置；父错误未处置时只有父错误进入未处置观测。
    pub fn with_source(mut self, source: Error) -> Self {
        source.disposition.store(true, Ordering::Release);
        self.source = Some(Box::new(source));
        self
    }

    /// 把新原因追加到现有原因链尾部，不覆盖已经采集的中间失败。
    pub(crate) fn with_appended_source(mut self, source: Error) -> Self {
        source.disposition.store(true, Ordering::Release);
        let mut tail = &mut self.source;
        while let Some(error) = tail {
            tail = &mut error.source;
        }
        *tail = Some(Box::new(source));
        self
    }

    /// 递归寻找原因链中最底层的根因。
    pub fn root_cause(&self) -> &Error {
        let mut current = self;
        while let Some(ref source) = current.source {
            current = source;
        }
        current
    }

    /// 获取原因链的深度。
    pub fn depth(&self) -> usize {
        let mut d = 0;
        let mut current = &self.source;
        while let Some(source) = current {
            d += 1;
            current = &source.source;
        }
        d
    }

    // ── 诊断处置 ──

    /// 标记该错误已经过诊断通道处置（crate 内：报告、观察、入队或恢复
    /// 尝试的消费入口调用）；原因链一并标记。
    pub(crate) fn mark_observed(&self) {
        self.disposition.store(true, Ordering::Release);
        let mut source = &self.source;
        while let Some(error) = source {
            error.disposition.store(true, Ordering::Release);
            source = &error.source;
        }
    }

    // ── 访问器 ──

    /// 返回错误分类码。
    pub fn code(&self) -> Errc {
        self.code
    }

    /// 返回错误消息。
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 返回错误严重级别。
    pub fn severity(&self) -> ErrorSeverity {
        self.severity
    }

    /// 返回创建该错误的调用点源文件。
    pub fn file(&self) -> &'static str {
        self.file
    }

    /// 返回创建该错误的调用点行号。
    pub fn line(&self) -> u32 {
        self.line
    }

    /// 返回错误创建时间。
    pub fn timestamp(&self) -> SystemTime {
        self.timestamp
    }

    /// 获取上游原因（如果有）。
    pub fn source_error(&self) -> Option<&Error> {
        self.source.as_deref()
    }

    /// 设置新的严重度，返回修改后的错误。
    pub fn set_severity(mut self, severity: ErrorSeverity) -> Self {
        self.severity = severity;
        self
    }

    // ── 判断 ──

    /// 检查错误码是否为 None（无错误信号）。
    pub fn is_none(&self) -> bool {
        self.code == Errc::None
    }

    /// 检查是否携带实际错误值，与 `is_none` 互为反向。
    pub fn has_value(&self) -> bool {
        !self.is_none()
    }

    /// 检查错误码是否等于指定值。
    pub fn is(&self, code: Errc) -> bool {
        self.code == code
    }

    // ── 便利工厂方法 ──

    /// 创建参数无效错误，并记录调用位置。
    #[track_caller]
    pub fn invalid_arg(message: impl Into<String>) -> Self {
        Self::new(Errc::InvalidArgument, message)
    }

    /// 创建资源未找到错误，并记录调用位置。
    #[track_caller]
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(Errc::NotFound, message)
    }

    /// 创建状态无效错误，并记录调用位置。
    #[track_caller]
    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::new(Errc::InvalidState, message)
    }

    /// 创建功能尚未实现错误，并记录调用位置。
    #[track_caller]
    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::new(Errc::NotImplemented, message)
    }

    /// 创建输入输出错误，并记录调用位置。
    #[track_caller]
    pub fn io_error(message: impl Into<String>) -> Self {
        Self::new(Errc::IoError, message)
    }

    /// 创建写入失败错误，并记录调用位置。
    #[track_caller]
    pub fn write_failure(message: impl Into<String>) -> Self {
        Self::new(Errc::WriteFailure, message)
    }

    /// 创建未知错误，并记录调用位置。
    #[track_caller]
    pub fn unknown(message: impl Into<String>) -> Self {
        Self::new(Errc::Unknown, message)
    }

    // ── 格式化输出 ──

    /// 短格式：`[ERROR] invalid_argument: 参数错误 (src/main.rs:42)`
    pub fn short_what(&self) -> String {
        format!(
            "[{}] {}: {} ({}:{})",
            self.severity, self.code, self.message, self.file, self.line
        )
    }

    /// 完整格式：包含当前错误与全部原因链的类别、码、消息、位置。
    pub fn what(&self) -> String {
        let mut result = format!(
            "[{}] {}: {} ({}:{})",
            self.code.category(),
            self.code,
            self.message,
            self.file,
            self.line
        );
        let mut source = self.source.as_deref();
        let mut depth = 1;
        while let Some(error) = source {
            result.push('\n');
            result.push_str(&"  ".repeat(depth));
            result.push_str(&format!(
                "cause: [{}] {} ({}:{})",
                error.code, error.message, error.file, error.line
            ));
            source = error.source.as_deref();
            depth += 1;
        }
        result
    }
}

impl Default for Error {
    fn default() -> Self {
        Self::new(Errc::None, "")
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("code", &self.code)
            .field("message", &self.message)
            .field("severity", &self.severity)
            .field("file", &self.file)
            .field("line", &self.line)
            .field("depth", &self.depth())
            .finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.what())
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        let code = match e.kind() {
            std::io::ErrorKind::NotFound => Errc::FileNotFound,
            std::io::ErrorKind::PermissionDenied => Errc::AccessDenied,
            std::io::ErrorKind::ConnectionRefused => Errc::ConnectionRefused,
            std::io::ErrorKind::ConnectionReset => Errc::ConnectionReset,
            std::io::ErrorKind::ConnectionAborted => Errc::ConnectionReset,
            std::io::ErrorKind::TimedOut => Errc::Timeout,
            std::io::ErrorKind::Interrupted => Errc::Cancelled,
            std::io::ErrorKind::InvalidInput => Errc::InvalidArgument,
            std::io::ErrorKind::InvalidData => Errc::FormatError,
            std::io::ErrorKind::WriteZero => Errc::WriteFailure,
            std::io::ErrorKind::AlreadyExists => Errc::AlreadyExists,
            _ => Errc::IoError,
        };
        Self::new(code, e.to_string())
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code && self.message == other.message
    }
}

impl Eq for Error {}

impl std::hash::Hash for Error {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.code.hash(state);
        self.message.hash(state);
    }
}
