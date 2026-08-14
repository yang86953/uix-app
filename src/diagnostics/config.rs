use std::path::PathBuf;

/// 控制最终上报边界何时捕获回溯。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacktracePolicy {
    Disabled,
    FatalOnly,
    ErrorsAndFatal,
}

/// 用于创建一个运行时作用域 [`crate::diagnostics::Diagnostics`] 的不可变配置。
///
/// 单份报告的安全预算属于框架固定不变量，故意不可配置。
#[derive(Debug, Clone)]
pub struct DiagnosticsConfig {
    pub(crate) report_capacity: usize,
    pub(crate) crash_report_directory: Option<PathBuf>,
    pub(crate) backtrace: BacktracePolicy,
}

impl DiagnosticsConfig {
    /// 设置留存报告数量。零容量会被归一化为 1，保证每次成功的 `report`
    /// 调用都能留存其报告。
    pub fn report_capacity(mut self, capacity: usize) -> Self {
        self.report_capacity = capacity.max(1);
        self
    }

    /// 设置紧急崩溃报告路径使用的目录。
    pub fn crash_report_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.crash_report_directory = Some(directory.into());
        self
    }

    /// 设置最终边界的回溯捕获策略。
    pub fn backtrace(mut self, policy: BacktracePolicy) -> Self {
        self.backtrace = policy;
        self
    }
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            report_capacity: 256,
            crash_report_directory: None,
            backtrace: BacktracePolicy::FatalOnly,
        }
    }
}
