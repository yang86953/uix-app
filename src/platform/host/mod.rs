//! 宿主机信息与 OS 服务功能域。

// OS 能力采集与系统服务实现只在 host 功能域内部可见。
#[cfg(feature = "platform")]
pub(crate) mod providers;

/// 通用状态级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    /// 表示操作成功或状态正常。
    Success,
    /// 表示中性的提示信息。
    Info,
    /// 表示需要关注但尚未失败的状态。
    Warning,
    /// 表示操作失败或状态异常。
    Error,
}
