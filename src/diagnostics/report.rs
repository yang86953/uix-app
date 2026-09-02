use std::fmt;
use std::time::SystemTime;

use crate::core::{Errc, ErrorSeverity};

/// Identity of one report within a single [`Diagnostics`](super::Diagnostics)
/// lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReportId(pub(crate) u64);

impl fmt::Display for ReportId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

// 保留经清洗的源位置 schema，供诊断快照与后续导出组件使用。
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct ReportSite {
    pub(crate) file: String,
    pub(crate) line: u32,
}

// 保留错误链的完整结构化字段，避免当前 tracing 简化输出丢失信息。
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct ErrorCause {
    pub(crate) code: Errc,
    pub(crate) severity: ErrorSeverity,
    pub(crate) summary: String,
    pub(crate) site: ReportSite,
}

#[derive(Debug, Clone)]
pub(crate) struct ReportResource {
    pub(crate) kind: &'static str,
    pub(crate) id: String,
}

/// Framework-owned origin schema. The public reporting entry deliberately
/// supplies the fixed application origin instead of accepting arbitrary
/// context strings.
#[derive(Debug, Clone)]
pub(crate) struct ReportOrigin {
    pub(crate) target: &'static str,
    pub(crate) operation: Option<&'static str>,
    pub(crate) resource: Option<ReportResource>,
}

impl ReportOrigin {
    pub(crate) const fn application() -> Self {
        Self {
            target: "application",
            operation: None,
            resource: None,
        }
    }

    // 框架来源构造器：窗口操作、图形终态失败与字体初始化等框架自身
    // 失败分支经 crate 内 report_with_origin 上报时使用。
    pub(crate) const fn framework(target: &'static str, operation: &'static str) -> Self {
        Self {
            target,
            operation: Some(operation),
            resource: None,
        }
    }

    // 资源身份构造器：副窗创建/身份校验与图形终态等携带具体资源事实的
    // 框架报告使用。
    pub(crate) fn with_resource(
        mut self,
        kind: &'static str,
        opaque_id: impl Into<String>,
    ) -> Self {
        self.resource = Some(ReportResource {
            kind,
            id: opaque_id.into(),
        });
        self
    }
}

#[derive(Debug)]
pub(crate) struct ReportDraft {
    pub(crate) observed_at: SystemTime,
    pub(crate) code: Errc,
    pub(crate) severity: ErrorSeverity,
    pub(crate) summary: String,
    pub(crate) causes: Vec<ErrorCause>,
    pub(crate) error_site: ReportSite,
    pub(crate) report_site: ReportSite,
    pub(crate) origin_target: String,
    pub(crate) operation: Option<String>,
    pub(crate) resource: Option<(String, String)>,
    pub(crate) thread: String,
    pub(crate) backtrace: Option<String>,
    pub(crate) causes_truncated: bool,
}

/// Bounded, sanitized observation generated from a typed framework error.
///
/// It intentionally does not retain the original `Error`.
// 报告对象保留完整快照字段，公共 getter 只暴露当前稳定的安全子集。
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ErrorReport {
    pub(crate) id: ReportId,
    pub(crate) observed_at: SystemTime,
    pub(crate) code: Errc,
    pub(crate) severity: ErrorSeverity,
    pub(crate) summary: String,
    pub(crate) causes: Vec<ErrorCause>,
    pub(crate) error_site: ReportSite,
    pub(crate) report_site: ReportSite,
    pub(crate) origin_target: String,
    pub(crate) operation: Option<String>,
    pub(crate) resource: Option<(String, String)>,
    pub(crate) thread: String,
    pub(crate) runtime_id: u64,
    pub(crate) backtrace: Option<String>,
    pub(crate) causes_truncated: bool,
    pub(crate) event_emitted: bool,
}

impl ErrorReport {
    pub(crate) fn from_draft(id: ReportId, runtime_id: u64, draft: ReportDraft) -> Self {
        Self {
            id,
            observed_at: draft.observed_at,
            code: draft.code,
            severity: draft.severity,
            summary: draft.summary,
            causes: draft.causes,
            error_site: draft.error_site,
            report_site: draft.report_site,
            origin_target: draft.origin_target,
            operation: draft.operation,
            resource: draft.resource,
            thread: draft.thread,
            runtime_id,
            backtrace: draft.backtrace,
            causes_truncated: draft.causes_truncated,
            event_emitted: false,
        }
    }

    /// 返回该报告在当前诊断运行时内的唯一标识。
    pub fn id(&self) -> ReportId {
        self.id
    }

    /// 返回错误被观测到的系统时间。
    pub fn observed_at(&self) -> SystemTime {
        self.observed_at
    }

    /// 返回错误分类码。
    pub fn code(&self) -> Errc {
        self.code
    }

    /// 返回错误严重级别。
    pub fn severity(&self) -> ErrorSeverity {
        self.severity
    }

    /// 返回经过清洗的错误摘要。
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// 返回报告来源的目标名称。
    pub fn origin_target(&self) -> &str {
        &self.origin_target
    }

    /// 返回可选的来源操作名称。
    pub fn operation(&self) -> Option<&str> {
        self.operation.as_deref()
    }

    /// 返回可选的资源种类及其不透明标识。
    pub fn resource(&self) -> Option<(&str, &str)> {
        self.resource
            .as_ref()
            .map(|(kind, id)| (kind.as_str(), id.as_str()))
    }

    /// 返回错误原因链是否因容量限制而被截断。
    pub fn causes_truncated(&self) -> bool {
        self.causes_truncated
    }

    /// 返回该报告是否已经成功发出诊断事件。
    pub fn event_emitted(&self) -> bool {
        self.event_emitted
    }

    /// 返回按回溯策略在最终上报边界捕获的有界回溯文本。
    ///
    /// 策略禁止捕获（如 `Disabled`）或严重级别低于策略门槛（如
    /// `FatalOnly` 下的非致命错误）时返回 `None`。
    pub fn backtrace(&self) -> Option<&str> {
        self.backtrace.as_deref()
    }
}

/// Immutable point-in-time view of one runtime's retained reports.
#[derive(Debug, Clone)]
pub struct DiagnosticsSnapshot {
    pub(crate) reports: Vec<ErrorReport>,
    pub(crate) total_reports: u64,
    pub(crate) evicted_reports: u64,
}

impl DiagnosticsSnapshot {
    /// 返回快照中仍被保留的错误报告。
    pub fn reports(&self) -> &[ErrorReport] {
        &self.reports
    }

    /// 返回该运行时累计接收的报告数，包括已淘汰报告。
    pub fn total_reports(&self) -> u64 {
        self.total_reports
    }

    /// 返回因保留容量限制而被淘汰的报告数。
    pub fn evicted_reports(&self) -> u64 {
        self.evicted_reports
    }
}
