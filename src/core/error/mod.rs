//! 错误码、严重级别、Error 结构体与 Result 扩展。
//!
//! 框架级错误词汇与基础错误类型。所有层使用统一的错误码枚举和严重级别。
//! Error 结构体携带码/消息/严重度/源码位置/时间戳/可选原因链。

mod codes;
pub(crate) mod result;
mod severity;
mod types;

pub use codes::Errc;
pub use result::{
    collect_errors, collect_values, make_error, to_std_error_code, try_invoke, Result,
    ResultErrorExt, ResultExt, ResultVoidExt,
};
pub use severity::ErrorSeverity;
pub use types::Error;

/// 类似 ? 操作符，但用于 UIX Result 类型。
#[macro_export]
macro_rules! uix_try {
    ($expr:expr_2021) => {
        match $expr {
            Ok(v) => v,
            Err(e) => return Err(e),
        }
    };
}

/// 类似 ? 操作符，自动将 std Result 转换为 UIX Result。
#[macro_export]
macro_rules! uix_try_std {
    ($expr:expr_2021) => {
        match $expr {
            Ok(v) => v,
            Err(e) => return Err(e.into()),
        }
    };
}
