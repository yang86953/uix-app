// 框架级错误码枚举

use std::fmt;

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
    WouldBlock = 13,

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
    GraphicsSurfaceLost = 504,
    GraphicsDeviceLost = 505,
    GraphicsOutOfMemory = 506,
    GraphicsOccluded = 507,

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
            Self::WouldBlock => Some(std::io::ErrorKind::WouldBlock),
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
            Errc::WouldBlock => "would_block",
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
            Errc::GraphicsSurfaceLost => "graphics_surface_lost",
            Errc::GraphicsDeviceLost => "graphics_device_lost",
            Errc::GraphicsOutOfMemory => "graphics_out_of_memory",
            Errc::GraphicsOccluded => "graphics_occluded",
            Errc::AppDomainBase => "app_domain_base",
        };
        write!(f, "{}", name)
    }
}
