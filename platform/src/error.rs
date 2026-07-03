// ============================================================================
// core/error.rs — 错误码、严重级别、Error 结构体与 Result 扩展
//
// 框架级错误词汇与基础错误类型。所有层使用统一的错误码枚举和严重级别。
// Error 结构体携带码/消息/严重度/源码位置/时间戳/可选原因链。
// diag 层在此基础之上构建更丰富的日志与收集能力。
// ============================================================================

use std::fmt;
use std::time::SystemTime;

// ════════════════════════════════════════════════════════════════════════════
// ErrorSeverity — 错误严重级别
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ErrorSeverity {
    Info = 0,
    Warning = 1,
    Error = 2,
    Fatal = 3,
}

impl ErrorSeverity {
    pub fn is_fatal(self) -> bool {
        matches!(self, Self::Fatal)
    }

    pub fn should_abort(self) -> bool {
        self >= Self::Fatal
    }
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
            Self::Fatal => write!(f, "FATAL"),
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Errc — 错误码枚举
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum Errc {
    None = 0,
    Unknown = 1,
    InvalidArgument = 2,
    OutOfRange = 3,
    NotFound = 4,
    AlreadyExists = 5,
    PermissionDenied = 6,
    Timeout = 7,
    Cancelled = 8,
    NotImplemented = 9,
    InvalidOperation = 10,
    InsufficientResources = 11,
    BadWeakPointer = 12,

    IoError = 100,
    FileNotFound = 101,
    AccessDenied = 102,
    FileBusy = 103,
    WriteFailure = 104,
    ReadFailure = 105,
    EndOfFile = 106,

    NetworkError = 200,
    ConnectionRefused = 201,
    ConnectionReset = 202,
    ConnectionTimeout = 203,
    DnsLookupFailed = 204,
    ProtocolViolation = 205,
    TlsError = 206,

    ProtocolError = 300,
    InvalidState = 301,
    FormatError = 302,
    ParseError = 303,
    SerializationError = 304,
    ChecksumMismatch = 305,

    DeadlockDetected = 400,
    TaskAbandoned = 401,
    FutureAlreadySatisfied = 402,

    PlatformError = 500,
    WindowCreationFailed = 501,
    ClassRegistrationFailed = 502,
    GdiOperationFailed = 503,

    AppDomainBase = 1000,
}

impl Errc {
    pub fn category(self) -> &'static str {
        let v = self as u32;
        if v < 100 {
            "general"
        } else if v < 200 {
            "io"
        } else if v < 300 {
            "network"
        } else if v < 400 {
            "protocol"
        } else if v < 500 {
            "concurrency"
        } else if v < 1000 {
            "platform"
        } else {
            "application"
        }
    }

    /// 尝试将 Errc 映射为 std::io::ErrorKind。
    pub fn to_io_kind(self) -> Option<std::io::ErrorKind> {
        match self {
            Self::InvalidArgument => Some(std::io::ErrorKind::InvalidInput),
            Self::NotFound | Self::FileNotFound => Some(std::io::ErrorKind::NotFound),
            Self::PermissionDenied | Self::AccessDenied => {
                Some(std::io::ErrorKind::PermissionDenied)
            }
            Self::Cancelled => Some(std::io::ErrorKind::Interrupted),
            Self::Timeout | Self::ConnectionTimeout => Some(std::io::ErrorKind::TimedOut),
            Self::ConnectionRefused => Some(std::io::ErrorKind::ConnectionRefused),
            Self::ConnectionReset => Some(std::io::ErrorKind::ConnectionReset),
            Self::AlreadyExists => Some(std::io::ErrorKind::AlreadyExists),
            Self::OutOfRange => Some(std::io::ErrorKind::InvalidData),
            Self::WriteFailure => Some(std::io::ErrorKind::WriteZero),
            _ => None,
        }
    }
}

impl fmt::Display for Errc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Errc::None => "none",
            Errc::Unknown => "unknown",
            Errc::InvalidArgument => "invalid_argument",
            Errc::OutOfRange => "out_of_range",
            Errc::NotFound => "not_found",
            Errc::AlreadyExists => "already_exists",
            Errc::PermissionDenied => "permission_denied",
            Errc::Timeout => "timeout",
            Errc::Cancelled => "cancelled",
            Errc::NotImplemented => "not_implemented",
            Errc::InvalidOperation => "invalid_operation",
            Errc::InsufficientResources => "insufficient_resources",
            Errc::BadWeakPointer => "bad_weak_pointer",
            Errc::IoError => "io_error",
            Errc::FileNotFound => "file_not_found",
            Errc::AccessDenied => "access_denied",
            Errc::FileBusy => "file_busy",
            Errc::WriteFailure => "write_failure",
            Errc::ReadFailure => "read_failure",
            Errc::EndOfFile => "end_of_file",
            Errc::NetworkError => "network_error",
            Errc::ConnectionRefused => "connection_refused",
            Errc::ConnectionReset => "connection_reset",
            Errc::ConnectionTimeout => "connection_timeout",
            Errc::DnsLookupFailed => "dns_lookup_failed",
            Errc::ProtocolViolation => "protocol_violation",
            Errc::TlsError => "tls_error",
            Errc::ProtocolError => "protocol_error",
            Errc::InvalidState => "invalid_state",
            Errc::FormatError => "format_error",
            Errc::ParseError => "parse_error",
            Errc::SerializationError => "serialization_error",
            Errc::ChecksumMismatch => "checksum_mismatch",
            Errc::DeadlockDetected => "deadlock_detected",
            Errc::TaskAbandoned => "task_abandoned",
            Errc::FutureAlreadySatisfied => "future_already_satisfied",
            Errc::PlatformError => "platform_error",
            Errc::WindowCreationFailed => "window_creation_failed",
            Errc::ClassRegistrationFailed => "class_registration_failed",
            Errc::GdiOperationFailed => "gdi_operation_failed",
            Errc::AppDomainBase => "app_domain_base",
        };
        write!(f, "{}", name)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Error — 框架级错误类型
//
// 携带错误码、严重级别、人类可读消息、源码位置、时间戳和可选原因链。
// 轻量设计，无外部依赖（使用 std::time::SystemTime 记录时间戳）。
// diag 层在此基础上提供更丰富的日志/收集/回溯能力。
// ════════════════════════════════════════════════════════════════════════════

/// 框架级错误类型。
///
/// 所有层统一使用此类型传递错误。携带错误码、严重级别、消息文本、
/// 源码位置（file/line）、时间戳和可选的上游原因链。
///
/// # 构造示例
///
/// ```ignore
/// let err = Error::new(Errc::NotFound, "资源未找到");
/// let err = Error::fatal(Errc::OutOfRange, "数组越界");
/// let err = Error::invalid_arg("参数 id 不能为空");
/// ```
#[derive(Clone)]
pub struct Error {
    code: Errc,
    message: String,
    severity: ErrorSeverity,
    file: &'static str,
    line: u32,
    timestamp: SystemTime,
    source: Option<Box<Error>>,
}

impl Error {
    // ── 构造器 ──

    /// 创建一个默认严重度为 Error 的框架错误。
    pub fn new(code: Errc, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            severity: ErrorSeverity::Error,
            file: file!(),
            line: line!(),
            timestamp: SystemTime::now(),
            source: None,
        }
    }

    /// 创建一个指定严重度的错误。
    pub fn with_severity(code: Errc, message: impl Into<String>, severity: ErrorSeverity) -> Self {
        Self {
            code,
            message: message.into(),
            severity,
            file: file!(),
            line: line!(),
            timestamp: SystemTime::now(),
            source: None,
        }
    }

    /// 创建一个 Warning 级别的错误。
    pub fn warn(code: Errc, message: impl Into<String>) -> Self {
        Self::with_severity(code, message, ErrorSeverity::Warning)
    }

    /// 创建一个 Info 级别的错误。
    pub fn info(code: Errc, message: impl Into<String>) -> Self {
        Self::with_severity(code, message, ErrorSeverity::Info)
    }

    /// 创建一个 Fatal 级别的错误。
    pub fn fatal(code: Errc, message: impl Into<String>) -> Self {
        Self::with_severity(code, message, ErrorSeverity::Fatal)
    }

    /// 创建一个带有自定义源码位置信息的错误。
    /// 在宏或包装函数中捕获调用方位置时使用。
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
        }
    }

    // ── 原因链 ──

    /// 为此错误附加一个上游原因。
    pub fn with_source(mut self, source: Error) -> Self {
        self.source = Some(Box::new(source));
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
        while let Some(ref source) = current {
            d += 1;
            current = &source.source;
        }
        d
    }

    // ── 访问器 ──

    pub fn code(&self) -> Errc {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn severity(&self) -> ErrorSeverity {
        self.severity
    }

    pub fn file(&self) -> &'static str {
        self.file
    }

    pub fn line(&self) -> u32 {
        self.line
    }

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

    /// has_value 是 is_none 的反向，语义为「没有错误」。
    pub fn has_value(&self) -> bool {
        self.code == Errc::None
    }

    /// 检查错误码是否等于指定值。
    pub fn is(&self, code: Errc) -> bool {
        self.code == code
    }

    // ── 便利工厂方法 ──

    pub fn invalid_arg(message: impl Into<String>) -> Self {
        Self::new(Errc::InvalidArgument, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(Errc::NotFound, message)
    }

    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::new(Errc::InvalidState, message)
    }

    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::new(Errc::NotImplemented, message)
    }

    pub fn io_error(message: impl Into<String>) -> Self {
        Self::new(Errc::IoError, message)
    }

    pub fn write_failure(message: impl Into<String>) -> Self {
        Self::new(Errc::WriteFailure, message)
    }

    pub fn unknown(message: impl Into<String>) -> Self {
        Self::new(Errc::Unknown, message)
    }

    // ── 格式化输出 ──

    /// 短格式：[ERROR] invalid_argument (src/main.rs:42)
    pub fn short_what(&self) -> String {
        format!(
            "[{}] {} ({}:{})",
            self.severity, self.code, self.file, self.line
        )
    }

    /// 完整格式：包含类别、码、消息、位置。
    pub fn what(&self) -> String {
        let mut result = format!(
            "[{}] {}: {} ({}:{})",
            self.code.category(),
            self.code,
            self.message,
            self.file,
            self.line
        );
        if let Some(ref source) = self.source {
            result.push_str(&format!(
                "\n  cause: [{}] {} ({}:{})",
                source.code, source.message, source.file, source.line
            ));
        }
        result
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Error Trait 实现
// ════════════════════════════════════════════════════════════════════════════

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
            std::io::ErrorKind::InvalidData => Errc::OutOfRange,
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

/// 手动实现 Hash，仅基于 code 和 message（与 PartialEq 一致）。
impl std::hash::Hash for Error {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.code.hash(state);
        self.message.hash(state);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Result — 框架级 Result 类型别名
// ════════════════════════════════════════════════════════════════════════════

/// UIX Result 类型别名 — 标准 Result 配合 uix-core::Error。
pub type Result<T, E = Error> = std::result::Result<T, E>;

// ════════════════════════════════════════════════════════════════════════════
// ResultExt — 对 Result<T, Error> 的扩展方法
// ════════════════════════════════════════════════════════════════════════════

/// 为 `Result<T, Error>` 提供 UIX 特定的便捷方法。
pub trait ResultExt<T> {
    fn has_value(&self) -> bool;
    fn has_error(&self) -> bool;
    fn value(&self) -> Option<&T>;
    fn value_mut(&mut self) -> Option<&mut T>;
    fn into_value(self) -> Option<T>;
    fn error(&self) -> Option<&Error>;
    fn value_or<U>(&self, default: U) -> T
    where
        T: Clone,
        U: Into<T>;
    fn err_message(&self) -> Option<&str>;
    fn err_code(&self) -> Option<Errc>;
}

impl<T> ResultExt<T> for Result<T, Error> {
    fn has_value(&self) -> bool {
        self.is_ok()
    }

    fn has_error(&self) -> bool {
        self.is_err()
    }

    fn value(&self) -> Option<&T> {
        self.as_ref().ok()
    }

    fn value_mut(&mut self) -> Option<&mut T> {
        self.as_mut().ok()
    }

    fn into_value(self) -> Option<T> {
        self.ok()
    }

    fn error(&self) -> Option<&Error> {
        self.as_ref().err()
    }

    fn value_or<U>(&self, default: U) -> T
    where
        T: Clone,
        U: Into<T>,
    {
        match self {
            Ok(v) => v.clone(),
            Err(_) => default.into(),
        }
    }

    fn err_message(&self) -> Option<&str> {
        match self {
            Err(e) => Some(e.message()),
            Ok(_) => None,
        }
    }

    fn err_code(&self) -> Option<Errc> {
        match self {
            Err(e) => Some(e.code()),
            Ok(_) => None,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ResultErrorExt — 便捷构造 Result<T, Error>
// ════════════════════════════════════════════════════════════════════════════

pub trait ResultErrorExt<T> {
    fn ok_value(value: T) -> Self;
    fn err_error(err: Error) -> Self;
    fn fail(code: Errc, message: impl Into<String>) -> Self;
}

impl<T> ResultErrorExt<T> for Result<T, Error> {
    fn ok_value(value: T) -> Self {
        Ok(value)
    }

    fn err_error(err: Error) -> Self {
        Err(err)
    }

    fn fail(code: Errc, message: impl Into<String>) -> Self {
        Err(Error::new(code, message))
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ResultVoidExt — 对 Result<(), Error> 的便捷构造
// ════════════════════════════════════════════════════════════════════════════

pub trait ResultVoidExt {
    fn ok_void() -> Self;
}

impl ResultVoidExt for Result<(), Error> {
    fn ok_void() -> Self {
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 工厂函数
// ════════════════════════════════════════════════════════════════════════════

pub fn make_error(code: Errc, message: impl Into<String>) -> Error {
    Error::new(code, message)
}

/// 将 Errc 转换为 std::io::ErrorKind（如果存在对应映射）。
pub fn to_std_error_code(code: Errc) -> Option<std::io::ErrorKind> {
    code.to_io_kind()
}

/// 尝试执行闭包，将 panic 转换为 Result。
pub fn try_invoke<F, T>(f: F) -> Result<T, Error>
where
    F: FnOnce() -> T,
{
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => Ok(v),
        Err(_) => Err(Error::new(Errc::Unknown, "function panicked")),
    }
}

/// 从迭代器中收集所有成功值。
pub fn collect_values<T, E>(
    results: impl IntoIterator<Item = std::result::Result<T, E>>,
) -> Vec<T> {
    results.into_iter().filter_map(|r| r.ok()).collect()
}

/// 从迭代器中收集所有错误。
pub fn collect_errors<T, E>(
    results: impl IntoIterator<Item = std::result::Result<T, E>>,
) -> Vec<E> {
    results.into_iter().filter_map(|r| r.err()).collect()
}

// ════════════════════════════════════════════════════════════════════════════
// 宏
// ════════════════════════════════════════════════════════════════════════════

/// 类似 ? 操作符，但用于 UIX Result 类型。
#[macro_export]
macro_rules! uix_try {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => return Err(e),
        }
    };
}

/// 类似 ? 操作符，自动将 std Result 转换为 UIX Result。
#[macro_export]
macro_rules! uix_try_std {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => return Err(e.into()),
        }
    };
}
