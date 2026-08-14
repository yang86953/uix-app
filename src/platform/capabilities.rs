//! 平台公开面的能力状态契约 — 状态级别。

/// 通用状态级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Success,
    Info,
    Warning,
    Error,
}
