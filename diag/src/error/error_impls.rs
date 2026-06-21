use crate::error::*;
use std::fmt;
use std::hash::{Hash, Hasher};

// ════════════════════════════════════════════════════════════════════════════
// Default / Display / Error trait impls
// ════════════════════════════════════════════════════════════════════════════

impl Default for Error {
    fn default() -> Self {
        Self::new(Errc::None, "")
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.what())
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::new(Errc::IoError, e.to_string())
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        self.inner.code == other.inner.code && self.inner.message == other.inner.message
    }
}

impl Eq for Error {}

impl Hash for Error {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.code.hash(state);
        self.inner.message.hash(state);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 工厂函数
// ════════════════════════════════════════════════════════════════════════════

pub fn make_error(code: Errc, message: impl Into<String>) -> Error {
    Error::new(code, message)
}

pub fn to_std_error_code(code: Errc) -> Option<std::io::ErrorKind> {
    match code {
        Errc::InvalidArgument => Some(std::io::ErrorKind::InvalidInput),
        Errc::NotFound | Errc::FileNotFound => Some(std::io::ErrorKind::NotFound),
        Errc::PermissionDenied | Errc::AccessDenied => Some(std::io::ErrorKind::PermissionDenied),
        Errc::Cancelled => Some(std::io::ErrorKind::Interrupted),
        Errc::Timeout | Errc::ConnectionTimeout => Some(std::io::ErrorKind::TimedOut),
        Errc::ConnectionRefused => Some(std::io::ErrorKind::ConnectionRefused),
        Errc::ConnectionReset => Some(std::io::ErrorKind::ConnectionReset),
        Errc::AlreadyExists => Some(std::io::ErrorKind::AlreadyExists),
        Errc::OutOfRange => Some(std::io::ErrorKind::InvalidData),
        Errc::WriteFailure => Some(std::io::ErrorKind::WriteZero),
        _ => None,
    }
}
