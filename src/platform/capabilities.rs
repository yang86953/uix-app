//! 平台公开面的能力状态契约 — 状态级别。

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
