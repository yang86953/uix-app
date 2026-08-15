//! 错误严重级别。

use std::fmt;

/// 用于分类诊断影响程度的有序严重度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ErrorSeverity {
    /// 不影响操作结果的信息提示。
    Info = 0,
    /// 操作可继续但需要关注的警告。
    Warning = 1,
    /// 当前操作失败但进程仍可恢复的错误。
    Error = 2,
    /// 调用方应终止当前进程或不可恢复流程的致命错误。
    Fatal = 3,
}

impl ErrorSeverity {
    /// 判断当前严重度是否为致命错误。
    pub fn is_fatal(self) -> bool {
        matches!(self, Self::Fatal)
    }

    /// 判断当前严重度是否要求责任边界中止执行。
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
