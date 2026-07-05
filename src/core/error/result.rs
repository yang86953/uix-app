// Result 类型别名与扩展 trait

use super::codes::Errc;
use super::types::Error;

/// UIX Result 类型别名 — 标准 Result 配合 uix-core::Error。
pub type Result<T, E = Error> = std::result::Result<T, E>;

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

pub trait ResultVoidExt {
    fn ok_void() -> Self;
}

impl ResultVoidExt for Result<(), Error> {
    fn ok_void() -> Self {
        Ok(())
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::error::ResultExt;

    #[test]
    fn try_invoke_panic_returns_error() {
        let result = try_invoke(|| -> i32 {
            panic!("intentional panic in test");
        });
        assert!(result.has_error());
        assert_eq!(result.err_code(), Some(Errc::Unknown));
    }
}
