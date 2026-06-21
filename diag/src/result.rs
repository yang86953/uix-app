// ============================================================================
// uix-diag/src/result.rs — Type alias and extension traits for Result<T, Error>
// ============================================================================

use crate::error::{Error, Errc};

/// UIX Result type alias — standard Result with UIX Error.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Extension trait adding UIX-specific methods to `Result<T, Error>`.
pub trait ResultExt<T> {
    fn has_value(&self) -> bool;
    fn has_error(&self) -> bool;
    fn value(&self) -> Option<&T>;
    fn value_mut(&mut self) -> Option<&mut T>;
    fn into_value(self) -> Option<T>;
    fn error(&self) -> Option<&Error>;
    fn value_or<U>(&self, default: U) -> T where T: Clone, U: Into<T>;
    fn err_message(&self) -> Option<&str>;
    fn err_code(&self) -> Option<Errc>;
}

impl<T> ResultExt<T> for Result<T, Error> {
    fn has_value(&self) -> bool { self.is_ok() }
    fn has_error(&self) -> bool { self.is_err() }
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
    fn value_or<U>(&self, default: U) -> T where T: Clone, U: Into<T> {
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

/// Additional helpers for `Result<T, Error>`.
pub trait ResultErrorExt<T> {
    fn ok_value(value: T) -> Self;
    fn err_error(err: Error) -> Self;
    fn fail(code: Errc, message: impl Into<String>) -> Self;
}

impl<T> ResultErrorExt<T> for Result<T, Error> {
    fn ok_value(value: T) -> Self { Ok(value) }
    fn err_error(err: Error) -> Self { Err(err) }
    fn fail(code: Errc, message: impl Into<String>) -> Self { Err(Error::new(code, message)) }
}

/// Helper for `Result<(), Error>`.
pub trait ResultVoidExt {
    fn ok_void() -> Self;
}

impl ResultVoidExt for Result<(), Error> {
    fn ok_void() -> Self { Ok(()) }
}

// ════════════════════════════════════════════════════════════════════════════
// try_invoke — catch panics and convert to Result
// ════════════════════════════════════════════════════════════════════════════

pub fn try_invoke<F, T>(f: F) -> Result<T, Error>
where
    F: FnOnce() -> T,
{
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .map(Ok)
        .unwrap_or_else(|_| Err(Error::new(Errc::Unknown, "function panicked")))
}

// ════════════════════════════════════════════════════════════════════════════
// Collection helpers
// ════════════════════════════════════════════════════════════════════════════

pub fn collect_values<T, E>(results: impl IntoIterator<Item = std::result::Result<T, E>>) -> Vec<T> {
    results.into_iter().filter_map(|r| r.ok()).collect()
}

pub fn collect_errors<T, E>(results: impl IntoIterator<Item = std::result::Result<T, E>>) -> Vec<E> {
    results.into_iter().filter_map(|r| r.err()).collect()
}

// ════════════════════════════════════════════════════════════════════════════
// try_invoke macro
// ════════════════════════════════════════════════════════════════════════════

#[macro_export]
macro_rules! uix_try {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => return Err(e),
        }
    };
}

#[macro_export]
macro_rules! uix_try_std {
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => return Err(e.into()),
        }
    };
}
