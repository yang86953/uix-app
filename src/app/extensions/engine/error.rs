//! Scheme 扩展引擎的类型化错误。
//!
//! 引擎任何失败都以稳定错误值返回：不 panic、不把语言细节泄漏进日志文本。
//! variant 刻意保持机器可判读；宿主层按[诊断契约]只消费其中的终态子集，
//! 脚本域错误先作为结果交给调用方。Display 手写实现，不引入 thiserror。

use std::fmt;

/// 脚本读取、求值或资源边界失败的类型化原因。
#[derive(Debug, Clone, PartialEq)]
pub enum SchemeError {
    // 读取器
    SourceTooLarge { actual: usize, maximum: usize },
    UnexpectedCharacter {
        line: u32,
        column: u32,
        character: char,
    },
    UnexpectedEof { line: u32, column: u32 },
    UnterminatedString { line: u32, column: u32 },
    UnterminatedList { line: u32, column: u32 },
    ReaderDepthExceeded { maximum: u32 },
    UnsupportedSyntax {
        line: u32,
        column: u32,
        feature: &'static str,
    },

    // 求值
    UnboundVariable { name: String },
    NotCallable,
    ArityMismatch {
        procedure: String,
        expected: String,
        actual: usize,
    },
    InvalidSyntax {
        form: &'static str,
        reason: &'static str,
    },
    WrongType {
        operation: &'static str,
        expected: &'static str,
    },
    IndexOutOfRange,
    NotImplemented { feature: &'static str },
    /// 脚本 `error` 过程抛出且未被 `guard` 捕获。
    UserError { message: String },
    /// 脚本 `raise` 抛出任意对象且未被捕获；携带 write 形式摘要。
    UncapturedRaise { summary: String },
    /// 机器内异常传播：携带被 raise 的原对象，由 handler 帧拦截；
    /// 传播到顶层后由 `run_program` 转为 `UncapturedRaise`。
    Raised(super::value::Value),

    // 库系统
    LibraryNotFound { name: String },
    InvalidLibrary { reason: &'static str },
    IncludeNotFound { path: String },

    // 资源与取消
    FuelExhausted,
    DepthLimitExceeded { maximum: u32 },
    AllocationQuotaExceeded { maximum: u64 },
    StringQuotaExceeded { maximum: u64 },
    /// 单次分配字节超限（make-vector / make-string / bytevector 等）。
    SingleAllocationTooLarge { requested: usize, maximum: usize },
    /// 标记-清扫回收后存活堆字节仍超上限。
    HeapQuotaExceeded { live: u64, maximum: u64 },
    Cancelled,
    /// 求值墙钟时间超限；与燃料、取消同为不可捕获的宿主边界。
    WallClockExceeded { milliseconds: u64 },

    // 数值
    DivisionByZero,
    ArithmeticOverflow,
    InvalidNumber { text: String },

    // 能力注入与宿主函数
    CapabilityNotDeclared { name: String },
    InvalidCapabilityName { name: String },
    HostFunctionError { name: String, message: String },

    // 兜底
    PanicCaught,
    EnginePoisoned,
}

impl fmt::Display for SchemeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemeError::SourceTooLarge { actual, maximum } => {
                write!(formatter, "脚本源码为 {actual} 字节，超过 {maximum} 字节上限")
            }
            SchemeError::UnexpectedCharacter {
                line,
                column,
                character,
            } => write!(
                formatter,
                "第 {line} 行第 {column} 列出现意外字符 {character:?}"
            ),
            SchemeError::UnexpectedEof { line, column } => {
                write!(formatter, "第 {line} 行第 {column} 列：源码意外结束")
            }
            SchemeError::UnterminatedString { line, column } => {
                write!(formatter, "第 {line} 行第 {column} 列：字符串字面量未终止")
            }
            SchemeError::UnterminatedList { line, column } => {
                write!(formatter, "第 {line} 行第 {column} 列：列表或向量未闭合")
            }
            SchemeError::ReaderDepthExceeded { maximum } => {
                write!(formatter, "读取器嵌套深度超过 {maximum} 上限")
            }
            SchemeError::UnsupportedSyntax {
                line,
                column,
                feature,
            } => write!(
                formatter,
                "第 {line} 行第 {column} 列：{feature} 在当前宿主环境未支持"
            ),
            SchemeError::UnboundVariable { name } => {
                write!(formatter, "未绑定变量 {name}")
            }
            SchemeError::NotCallable => write!(formatter, "求值结果不是可调用过程"),
            SchemeError::ArityMismatch {
                procedure,
                expected,
                actual,
            } => write!(
                formatter,
                "过程 {procedure} 参数数量不符：期望 {expected}，实际 {actual}"
            ),
            SchemeError::InvalidSyntax { form, reason } => {
                write!(formatter, "{form} 语法无效：{reason}")
            }
            SchemeError::WrongType { operation, expected } => {
                write!(formatter, "{operation} 需要 {expected}")
            }
            SchemeError::IndexOutOfRange => write!(formatter, "列表或向量索引越界"),
            SchemeError::NotImplemented { feature } => {
                write!(formatter, "{feature} 在当前宿主环境未实现")
            }
            SchemeError::UserError { message } => {
                write!(formatter, "脚本显式失败：{message}")
            }
            SchemeError::UncapturedRaise { summary } => {
                write!(formatter, "脚本异常未被捕获：{summary}")
            }
            SchemeError::Raised(value) => {
                write!(formatter, "脚本异常：{}", value.to_write_string())
            }
            SchemeError::LibraryNotFound { name } => {
                write!(formatter, "库 {name} 未定义")
            }
            SchemeError::InvalidLibrary { reason } => {
                write!(formatter, "库声明无效：{reason}")
            }
            SchemeError::IncludeNotFound { path } => {
                write!(formatter, "include 来源 {path} 不在包声明内")
            }
            SchemeError::FuelExhausted => write!(formatter, "脚本燃料耗尽"),
            SchemeError::DepthLimitExceeded { maximum } => {
                write!(formatter, "求值深度超过 {maximum} 上限")
            }
            SchemeError::AllocationQuotaExceeded { maximum } => {
                write!(formatter, "脚本分配的值数量超过 {maximum} 配额")
            }
            SchemeError::StringQuotaExceeded { maximum } => {
                write!(formatter, "脚本字符串字节超过 {maximum} 配额")
            }
            SchemeError::SingleAllocationTooLarge { requested, maximum } => {
                write!(formatter, "单次分配 {requested} 字节超过 {maximum} 上限")
            }
            SchemeError::HeapQuotaExceeded { live, maximum } => {
                write!(formatter, "回收后存活堆 {live} 字节超过 {maximum} 配额")
            }
            SchemeError::Cancelled => write!(formatter, "脚本执行已取消"),
            SchemeError::WallClockExceeded { milliseconds } => {
                write!(formatter, "脚本执行超过 {milliseconds}ms 墙钟上限")
            }
            SchemeError::DivisionByZero => write!(formatter, "除数为零"),
            SchemeError::ArithmeticOverflow => write!(formatter, "定点运算溢出"),
            SchemeError::InvalidNumber { text } => {
                write!(formatter, "数值字面量或转换无效：{text}")
            }
            SchemeError::CapabilityNotDeclared { name } => write!(
                formatter,
                "宿主能力 {name} 未在清单 capabilities 中声明"
            ),
            SchemeError::InvalidCapabilityName { name } => {
                write!(formatter, "宿主能力名无效：{name}")
            }
            SchemeError::HostFunctionError { name, message } => {
                write!(formatter, "宿主能力 {name} 失败：{message}")
            }
            SchemeError::PanicCaught => {
                write!(formatter, "解释器内部 panic，已兜底为稳定失败")
            }
            SchemeError::EnginePoisoned => {
                write!(formatter, "解释器在 panic 后已污染，不能复用")
            }
        }
    }
}

impl std::error::Error for SchemeError {}
