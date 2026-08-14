// Result 类型别名与扩展 trait

use super::codes::Errc;
use super::types::Error;

/// UIX Result 类型别名 — 标准 Result 配合 uix-core::Error。
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// 为 `Result<T, Error>` 提供 UIX 特定的便捷方法。
pub trait ResultExt<T> {
    /// 返回结果是否包含成功值。
    fn has_value(&self) -> bool;
    /// 返回结果是否包含 UIX 错误。
    fn has_error(&self) -> bool;
    /// 借用成功值，错误结果返回 `None`。
    fn value(&self) -> Option<&T>;
    /// 可变借用成功值，错误结果返回 `None`。
    fn value_mut(&mut self) -> Option<&mut T>;
    /// 消耗结果并取得成功值，错误结果返回 `None`。
    fn into_value(self) -> Option<T>;
    /// 借用 UIX 错误，成功结果返回 `None`。
    fn error(&self) -> Option<&Error>;
    /// 克隆成功值，或在错误时把默认值转换为结果类型。
    fn value_or<U>(&self, default: U) -> T
    where
        T: Clone,
        U: Into<T>;
    /// 返回错误消息，成功结果返回 `None`。
    fn err_message(&self) -> Option<&str>;
    /// 返回错误代码，成功结果返回 `None`。
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

/// 为使用 UIX [`Error`] 的结果提供显式成功和失败构造器。
pub trait ResultErrorExt<T> {
    /// 从成功值构造结果。
    fn ok_value(value: T) -> Self;
    /// 从现有 UIX 错误构造失败结果。
    fn err_error(err: Error) -> Self;
    /// 从错误代码和消息构造失败结果。
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

/// 为无返回值的 UIX 结果提供成功构造器。
pub trait ResultVoidExt {
    /// 构造包含单元值的成功结果。
    fn ok_void() -> Self;
}

impl ResultVoidExt for Result<(), Error> {
    fn ok_void() -> Self {
        Ok(())
    }
}

/// 从错误代码和消息创建 UIX 错误。
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
        Err(payload) => {
            let message = if let Some(message) = payload.downcast_ref::<&str>() {
                format!("function panicked: {message}")
            } else if let Some(message) = payload.downcast_ref::<String>() {
                format!("function panicked: {message}")
            } else {
                "function panicked with a non-string payload".to_owned()
            };
            Err(Error::new(Errc::Unknown, message))
        }
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
