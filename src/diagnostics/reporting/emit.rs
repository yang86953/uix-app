//! Structured tracing emission Component owned by the reporting Module.

use std::cell::Cell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::ErrorSeverity;

use super::super::report::ErrorReport;

thread_local! {
    static EMITTING_REPORT: Cell<bool> = const { Cell::new(false) };
}

struct EmissionGuard;

impl EmissionGuard {
    fn enter() -> Option<Self> {
        EMITTING_REPORT.with(|emitting| {
            if emitting.replace(true) {
                None
            } else {
                Some(Self)
            }
        })
    }
}

impl Drop for EmissionGuard {
    fn drop(&mut self) {
        EMITTING_REPORT.with(|emitting| emitting.set(false));
    }
}

pub(super) fn emit_report(report: &ErrorReport, emergency_count: &AtomicU64) -> bool {
    let Some(_guard) = EmissionGuard::enter() else {
        emergency_notice(emergency_count, "recursive diagnostics event suppressed");
        return false;
    };

    match catch_unwind(AssertUnwindSafe(|| emit_event(report))) {
        Ok(()) => true,
        Err(_) => {
            emergency_notice(emergency_count, "diagnostics subscriber panicked");
            false
        }
    }
}

fn emit_event(report: &ErrorReport) {
    let report_id = report.id.0;
    let code = report.code.to_string();
    let severity = report.severity.to_string();
    let summary = report.summary.as_str();
    let runtime_id = report.runtime_id;
    let origin_target = report.origin_target.as_str();
    let operation = report.operation.as_deref().unwrap_or("");
    let (resource_kind, resource_id) = report
        .resource
        .as_ref()
        .map(|(kind, id)| (kind.as_str(), id.as_str()))
        .unwrap_or(("", ""));
    let fatal = report.severity == ErrorSeverity::Fatal;

    match report.severity {
        ErrorSeverity::Info => tracing::event!(
            target: "uix::diagnostics",
            tracing::Level::INFO,
            "uix.report_id" = report_id,
            "uix.error.code" = %code,
            "uix.error.severity" = %severity,
            "uix.error.summary" = %summary,
            "uix.error.fatal" = fatal,
            "uix.runtime_id" = runtime_id,
            "uix.origin.target" = %origin_target,
            "uix.operation" = %operation,
            "uix.resource.kind" = %resource_kind,
            "uix.resource.id" = %resource_id,
            "typed error reached its final responsibility boundary"
        ),
        ErrorSeverity::Warning => tracing::event!(
            target: "uix::diagnostics",
            tracing::Level::WARN,
            "uix.report_id" = report_id,
            "uix.error.code" = %code,
            "uix.error.severity" = %severity,
            "uix.error.summary" = %summary,
            "uix.error.fatal" = fatal,
            "uix.runtime_id" = runtime_id,
            "uix.origin.target" = %origin_target,
            "uix.operation" = %operation,
            "uix.resource.kind" = %resource_kind,
            "uix.resource.id" = %resource_id,
            "typed error reached its final responsibility boundary"
        ),
        ErrorSeverity::Error | ErrorSeverity::Fatal => tracing::event!(
            target: "uix::diagnostics",
            tracing::Level::ERROR,
            "uix.report_id" = report_id,
            "uix.error.code" = %code,
            "uix.error.severity" = %severity,
            "uix.error.summary" = %summary,
            "uix.error.fatal" = fatal,
            "uix.runtime_id" = runtime_id,
            "uix.origin.target" = %origin_target,
            "uix.operation" = %operation,
            "uix.resource.kind" = %resource_kind,
            "uix.resource.id" = %resource_id,
            "typed error reached its final responsibility boundary"
        ),
    }
}

fn emergency_notice(counter: &AtomicU64, message: &str) {
    let occurrence = counter.fetch_add(1, Ordering::Relaxed).saturating_add(1);
    if occurrence.is_power_of_two() {
        eprintln!("uix diagnostics emergency: {message}; occurrence={occurrence}");
    }
}
