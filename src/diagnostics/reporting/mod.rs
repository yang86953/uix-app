//! Diagnostics System 拥有的私有 reporting Module。
//!
//! 该 Module 把一个类型化错误转换为有界报告，负责留存，并发射生成的
//! 不可变观察结果。其嵌套的 `store` 与 `emit` 模块是 Component，
//! 不是公开或同级 Module。

use std::backtrace::Backtrace;
use std::fmt::{self, Write};
use std::panic::Location;
use std::sync::atomic::AtomicU64;
use std::sync::{Mutex, MutexGuard};
use std::time::SystemTime;

use std::sync::Arc;

use crate::core::{Error, ErrorSeverity};

use super::config::{BacktracePolicy, DiagnosticsConfig};
use super::report::{
    DiagnosticsSnapshot, ErrorCause, ErrorReport, ReportDraft, ReportId, ReportOrigin, ReportSite,
};

mod emit;
mod notify;
mod store;

pub use notify::ReportSubscription;
use emit::emit_report;
use notify::ReportNotifier;
use store::ReportStore;

const MAX_REPORT_BYTES: usize = 32 * 1024;
const MAX_SEGMENT_BYTES: usize = 2 * 1024;
const MAX_CAUSES: usize = 16;
const MAX_METADATA_TEXT_BYTES: usize = 256;
const RESERVED_FIXED_BYTES: usize = 2 * 1024;

/// 为一个 `Diagnostics` System 实例持有报告留存、结构化诊断发射与即时
/// 通知的私有 Module。
pub(super) struct ReportingModule {
    store: Mutex<ReportStore>,
    emergency_count: AtomicU64,
    notifier: Arc<ReportNotifier>,
}

impl ReportingModule {
    pub(super) fn new(config: &DiagnosticsConfig) -> Self {
        Self {
            store: Mutex::new(ReportStore::new(config.report_capacity)),
            emergency_count: AtomicU64::new(0),
            notifier: Arc::new(ReportNotifier::new()),
        }
    }

    pub(super) fn subscribe_report<F>(&self, handler: F) -> ReportSubscription
    where
        F: Fn(&ErrorReport) + Send + Sync + 'static,
    {
        self.notifier.subscribe(handler)
    }

    pub(super) fn snapshot(&self) -> DiagnosticsSnapshot {
        self.lock_store().snapshot()
    }

    pub(super) fn report(
        &self,
        runtime_id: u64,
        backtrace_policy: BacktracePolicy,
        error: &Error,
        origin: ReportOrigin,
        report_site: &'static Location<'static>,
    ) -> ReportId {
        // 构建有界草稿 → 入库 → 取出完整报告发射（发射失败不阻塞上报）。
        let draft = ReportDraftBuilder::new(backtrace_policy).build(error, origin, report_site);
        let id = self.lock_store().insert(runtime_id, draft);

        let report = self.lock_store().report(id).cloned();
        if let Some(report) = report {
            if emit_report(&report, &self.emergency_count) {
                self.lock_store().mark_event_emitted(id);
            }
            // 报告入库后立即同步通知订阅者：错误一发生宿主即可感知。
            self.notifier.notify(&report, &self.emergency_count);
        }
        id
    }

    fn lock_store(&self) -> MutexGuard<'_, ReportStore> {
        self.store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// 把类型化错误转换为有界、净化过的报告草稿的 Component。
/// 它不持有运行时状态，也无法访问 reporting store。
struct ReportDraftBuilder {
    backtrace_policy: BacktracePolicy,
}

impl ReportDraftBuilder {
    const fn new(backtrace_policy: BacktracePolicy) -> Self {
        Self { backtrace_policy }
    }

    fn build(
        &self,
        error: &Error,
        origin: ReportOrigin,
        report_site: &'static Location<'static>,
    ) -> ReportDraft {
        // 预留固定开销后得到正文预算，所有文本字段共用同一预算。
        let mut budget = TextBudget::new(MAX_REPORT_BYTES.saturating_sub(RESERVED_FIXED_BYTES));

        // 净化来源元数据：目标、操作与资源（资源含 kind/id 两项）。
        let origin_target = budget.sanitize(origin.target, MAX_METADATA_TEXT_BYTES).text;
        let operation = origin
            .operation
            .map(|value| budget.sanitize(value, MAX_METADATA_TEXT_BYTES).text);
        let resource = origin.resource.map(|resource| {
            (
                budget.sanitize(resource.kind, MAX_METADATA_TEXT_BYTES).text,
                budget.sanitize(&resource.id, MAX_METADATA_TEXT_BYTES).text,
            )
        });

        // 错误站点与报告调用站点都只取文件名与行号。
        let error_site = ReportSite {
            file: budget
                .sanitize(file_basename(error.file()), MAX_METADATA_TEXT_BYTES)
                .text,
            line: error.line(),
        };
        let report_site = ReportSite {
            file: budget
                .sanitize(file_basename(report_site.file()), MAX_METADATA_TEXT_BYTES)
                .text,
            line: report_site.line(),
        };
        let thread = budget
            .sanitize(&thread_summary(), MAX_METADATA_TEXT_BYTES)
            .text;
        let summary_result = budget.sanitize(error.message(), MAX_SEGMENT_BYTES);
        let summary = summary_result.text;
        let mut causes_truncated = summary_result.truncated;

        // 沿来源链收集成因，条数与字节预算都设上限。
        let mut causes = Vec::new();
        let mut source = error.source_error();
        while let Some(cause) = source {
            if causes.len() == MAX_CAUSES {
                causes_truncated = true;
                break;
            }
            if budget.remaining() == 0 {
                causes_truncated = true;
                break;
            }

            let cause_summary = budget.sanitize(cause.message(), MAX_SEGMENT_BYTES);
            causes_truncated |= cause_summary.truncated;
            let site = ReportSite {
                file: budget
                    .sanitize(file_basename(cause.file()), MAX_METADATA_TEXT_BYTES)
                    .text,
                line: cause.line(),
            };
            causes.push(ErrorCause {
                code: cause.code(),
                severity: cause.severity(),
                summary: cause_summary.text,
                site,
            });
            source = cause.source_error();
        }
        if source.is_some() {
            causes_truncated = true;
        }

        // 按策略捕获回溯，并写入剩余预算内（超出的部分被截断）。
        let backtrace = if should_capture_backtrace(self.backtrace_policy, error.severity()) {
            let backtrace = Backtrace::capture();
            let mut output = BoundedFormatter::new(budget.remaining());
            let _ = write!(&mut output, "{backtrace}");
            budget.consume(output.text.len());
            (!output.text.is_empty()).then_some(output.text)
        } else {
            None
        };

        ReportDraft {
            observed_at: SystemTime::now(),
            code: error.code(),
            severity: error.severity(),
            summary,
            causes,
            error_site,
            report_site,
            origin_target,
            operation,
            resource,
            thread,
            backtrace,
            causes_truncated,
        }
    }
}

fn should_capture_backtrace(policy: BacktracePolicy, severity: ErrorSeverity) -> bool {
    // 按策略分级决定是否在最终上报边界捕获回溯。
    match policy {
        BacktracePolicy::Disabled => false,
        BacktracePolicy::FatalOnly => severity == ErrorSeverity::Fatal,
        BacktracePolicy::ErrorsAndFatal => severity >= ErrorSeverity::Error,
    }
}

fn file_basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn thread_summary() -> String {
    let thread = std::thread::current();
    match thread.name() {
        Some(name) => format!("{name}:{:?}", thread.id()),
        None => format!("{:?}", thread.id()),
    }
}

struct SanitizedText {
    text: String,
    truncated: bool,
}

struct TextBudget {
    remaining: usize,
}

impl TextBudget {
    const fn new(remaining: usize) -> Self {
        Self { remaining }
    }

    const fn remaining(&self) -> usize {
        self.remaining
    }

    fn consume(&mut self, bytes: usize) {
        self.remaining = self.remaining.saturating_sub(bytes);
    }

    fn sanitize(&mut self, input: &str, segment_limit: usize) -> SanitizedText {
        // 段上限与剩余预算取较小者；控制字符替换为空格。
        let limit = segment_limit.min(self.remaining);
        let mut text = String::with_capacity(limit.min(input.len()));
        let mut truncated = false;

        for character in input.chars() {
            let character = if character.is_control() {
                ' '
            } else {
                character
            };
            // 超出上限即截断，且预算一并扣减已写入部分。
            let required = character.len_utf8();
            if text.len().saturating_add(required) > limit {
                truncated = true;
                break;
            }
            text.push(character);
        }
        if !truncated && text.len() < input.len() {
            truncated = true;
        }
        self.consume(text.len());
        SanitizedText { text, truncated }
    }
}

struct BoundedFormatter {
    text: String,
    limit: usize,
}

impl BoundedFormatter {
    fn new(limit: usize) -> Self {
        Self {
            text: String::new(),
            limit,
        }
    }
}

impl fmt::Write for BoundedFormatter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        // 控制字符替换为空格（保留换行），超过上限即停止写入。
        for character in value.chars() {
            let character = if character.is_control() && character != '\n' {
                ' '
            } else {
                character
            };
            if self.text.len().saturating_add(character.len_utf8()) > self.limit {
                break;
            }
            self.text.push(character);
        }
        Ok(())
    }
}
