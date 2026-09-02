//! 错误码、严重级别、Error 结构体与 Result 扩展。
//!
//! 框架级错误词汇与基础错误类型。所有层使用统一的错误码枚举和严重级别。
//! Error 结构体携带码/消息/严重度/源码位置/时间戳/可选原因链。

mod codes;
pub(crate) mod result;
mod severity;
mod types;
mod unhandled;

pub use unhandled::unhandled_error_summary;

pub use codes::Errc;
pub use result::{
    Result, ResultErrorExt, ResultExt, ResultVoidExt, collect_errors, collect_values, make_error,
    to_std_error_code, try_invoke,
};
pub use severity::ErrorSeverity;
pub use types::Error;

/// 类似 ? 操作符，但用于 UIX Result 类型。
#[macro_export]
macro_rules! uix_try {
    // Rust 2024 表达式片段同时接受 const 块与下划线表达式。
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
    // Rust 2024 表达式片段同时接受 const 块与下划线表达式。
    ($expr:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => return Err(e.into()),
        }
    };
}
