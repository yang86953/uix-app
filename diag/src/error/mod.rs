// ============================================================================
// uix-platform/src/error.rs — Rich error type with codes, source location, nesting
// ============================================================================

use chrono::{DateTime, Utc};
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

// ════════════════════════════════════════════════════════════════════════════
// 错误严重度
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
    pub fn is_fatal(self) -> bool { matches!(self, Self::Fatal) }
    pub fn should_abort(self) -> bool { self >= Self::Fatal }
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
// 错误码
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
        } else if v >= 1000 {
            "application"
        } else {
            "unknown"
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
// Error 内部实现
// ════════════════════════════════════════════════════════════════════════════

#[derive(Clone)]
struct ErrorImpl {
    code: Errc,
    message: String,
    severity: ErrorSeverity,
    file: &'static str,
    line: u32,
    timestamp: DateTime<Utc>,
    cause: Option<Arc<ErrorImpl>>,
    tags: Vec<(String, String)>,
}

impl fmt::Debug for ErrorImpl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorImpl")
            .field("code", &self.code)
            .field("message", &self.message)
            .field("file", &self.file)
            .field("line", &self.line)
            .finish()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Error — 公开错误类型
// ════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug)]
pub struct Error {
    inner: Arc<ErrorImpl>,
}

impl Error {
    pub fn new(code: Errc, message: impl Into<String>) -> Self {
        Self::with_severity(code, message, ErrorSeverity::Error)
    }

    pub fn warn(code: Errc, message: impl Into<String>) -> Self {
        Self::with_severity(code, message, ErrorSeverity::Warning)
    }

    pub fn info(code: Errc, message: impl Into<String>) -> Self {
        Self::with_severity(code, message, ErrorSeverity::Info)
    }

    pub fn fatal(code: Errc, message: impl Into<String>) -> Self {
        Self::with_severity(code, message, ErrorSeverity::Fatal)
    }

    pub fn with_severity(code: Errc, message: impl Into<String>, severity: ErrorSeverity) -> Self {
        Self {
            inner: Arc::new(ErrorImpl {
                code,
                message: message.into(),
                severity,
                file: file!(),
                line: line!(),
                timestamp: Utc::now(),
                cause: None,
                tags: Vec::new(),
            }),
        }
    }

    pub fn with_location(
        code: Errc,
        message: impl Into<String>,
        file: &'static str,
        line: u32,
    ) -> Self {
        Self {
            inner: Arc::new(ErrorImpl {
                code,
                message: message.into(),
                severity: ErrorSeverity::Error,
                file,
                line,
                timestamp: Utc::now(),
                cause: None,
                tags: Vec::new(),
            }),
        }
    }

    pub fn set_severity(mut self, severity: ErrorSeverity) -> Self {
        Arc::make_mut(&mut self.inner).severity = severity;
        self
    }

    pub fn severity(&self) -> ErrorSeverity { self.inner.severity }
    pub fn code(&self) -> Errc { self.inner.code }
    pub fn message(&self) -> &str { &self.inner.message }
    pub fn file(&self) -> &'static str { self.inner.file }
    pub fn line(&self) -> u32 { self.inner.line }
    pub fn timestamp(&self) -> DateTime<Utc> { self.inner.timestamp }

    pub fn has_value(&self) -> bool {
        self.inner.code == Errc::None
    }

    pub fn is(&self, code: Errc) -> bool {
        self.inner.code == code
    }

    pub fn with_cause(mut self, cause: Error) -> Self {
        let impl_ = Arc::make_mut(&mut self.inner);
        let cause_msg = cause.to_string();
        impl_.cause = Some(cause.inner.clone());
        impl_.message = format!("{} | caused by: {}", impl_.message, cause_msg);
        self
    }

    pub fn root_cause(&self) -> Self {
        let mut current = &self.inner;
        while let Some(ref cause) = current.cause {
            current = cause;
        }
        Self {
            inner: current.clone(),
        }
    }

    // ── 便利工厂方法 ──

    pub fn invalid_arg(msg: impl Into<String>) -> Self {
        Self::new(Errc::InvalidArgument, msg)
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(Errc::NotFound, msg)
    }

    pub fn invalid_state(msg: impl Into<String>) -> Self {
        Self::new(Errc::InvalidState, msg)
    }

    pub fn not_implemented(msg: impl Into<String>) -> Self {
        Self::new(Errc::NotImplemented, msg)
    }

    pub fn io_error(msg: impl Into<String>) -> Self {
        Self::new(Errc::IoError, msg)
    }

    pub fn write_failure(msg: impl Into<String>) -> Self {
        Self::new(Errc::WriteFailure, msg)
    }

    pub fn unknown(msg: impl Into<String>) -> Self {
        Self::new(Errc::Unknown, msg)
    }

    pub fn depth(&self) -> usize {
        let mut d = 0;
        let mut current = &self.inner.cause;
        while let Some(ref cause) = current {
            d += 1;
            current = &cause.cause;
        }
        d
    }

    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let impl_ = Arc::make_mut(&mut self.inner);
        impl_.tags.push((key.into(), value.into()));
        self
    }

    pub fn tag(&self, key: &str) -> Option<&str> {
        self.inner
            .tags
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn short_what(&self) -> String {
        format!(
            "[{}] {} ({}:{})",
            self.inner.code,
            if self.inner.message.is_empty() {
                self.inner.code.to_string()
            } else {
                self.inner.message.clone()
            },
            self.inner.file,
            self.inner.line
        )
    }

    pub fn what(&self) -> String {
        let ts = self.inner.timestamp.format("%H:%M:%S");
        let mut result = format!(
            "[{}] {}: {} ({}, {}:{})",
            self.inner.code.category(),
            self.inner.code,
            self.inner.message,
            ts,
            self.inner.file,
            self.inner.line
        );
        if !self.inner.tags.is_empty() {
            result.push_str(" {");
            for (i, (k, v)) in self.inner.tags.iter().enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&format!("{}={}", k, v));
            }
            result.push('}');
        }
        if let Some(ref cause) = self.inner.cause {
            result.push_str(&format!(
                "\n  cause: [{}] {} ({}:{})",
                cause.code, cause.message, cause.file, cause.line
            ));
        }
        result
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Trait impls + factory functions (split to error_impls.rs for ≤400 lines)
// ════════════════════════════════════════════════════════════════════════════

mod error_impls;
pub use error_impls::*;
