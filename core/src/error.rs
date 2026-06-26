// ============================================================================
// core/error.rs — 错误码与严重级别
//
// 框架级错误词汇。所有层使用统一的错误码枚举，diag 层在此基础上
// 构建 Error 结构体（带时间戳/上下文/链式 cause）。
// ============================================================================

use std::fmt;

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
