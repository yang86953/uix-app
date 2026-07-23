//! Runtime-scoped error observation shared by UIX and its applications.

mod config;
mod emit;
mod recovery;
mod report;
mod store;

use std::backtrace::Backtrace;
use std::fmt::{self, Write};
use std::panic::Location;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::core::{Error, ErrorSeverity};

pub use config::{BacktracePolicy, DiagnosticsConfig};
use emit::emit_report;
pub use recovery::{RecoveryAction, RecoveryOutcome, RecoverySubscription};
pub(crate) use report::ReportOrigin;
pub use report::{DiagnosticsSnapshot, ErrorReport, ReportId};
use report::{ErrorCause, ReportDraft, ReportSite};
use store::ReportStore;

const MAX_REPORT_BYTES: usize = 32 * 1024;
const MAX_SEGMENT_BYTES: usize = 2 * 1024;
const MAX_CAUSES: usize = 16;
const MAX_METADATA_TEXT_BYTES: usize = 256;
const RESERVED_FIXED_BYTES: usize = 2 * 1024;

static NEXT_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);

struct DiagnosticsInner {
    runtime_id: u64,
    config: DiagnosticsConfig,
    store: Mutex<ReportStore>,
    recovery: Arc<recovery::RecoveryRegistry>,
    emergency_count: AtomicU64,
}

/// Cloneable handle to one runtime's bounded error-report store.
#[derive(Clone)]
pub struct Diagnostics {
    inner: Arc<DiagnosticsInner>,
}

impl Diagnostics {
    pub fn new(config: DiagnosticsConfig) -> Self {
        let runtime_id = NEXT_RUNTIME_ID.fetch_add(1, Ordering::Relaxed);
        let store = ReportStore::new(config.report_capacity);
        Self {
            inner: Arc::new(DiagnosticsInner {
                runtime_id,
                config,
                store: Mutex::new(store),
                recovery: Arc::new(recovery::RecoveryRegistry::new()),
                emergency_count: AtomicU64::new(0),
            }),
        }
    }

    /// Reports an application-owned typed error at its final responsibility
    /// boundary.
    #[track_caller]
    pub fn report(&self, error: Error) -> ReportId {
        self.report_with_origin(error, ReportOrigin::application())
    }

    /// Returns an immutable point-in-time snapshot ordered by `ReportId`.
    pub fn snapshot(&self) -> DiagnosticsSnapshot {
        self.lock_store().snapshot()
    }

    /// Registers recovery logic for one exact typed error code.
    ///
    /// Handlers run synchronously, in stable registration order, only when
    /// [`Self::attempt_recovery`] is called from a safe owner/caller thread.
    /// Native callbacks must enqueue failures instead of invoking user code.
    pub fn on_error<F>(&self, code: crate::core::Errc, handler: F) -> RecoverySubscription
    where
        F: Fn(&Error) -> RecoveryAction + Send + Sync + 'static,
    {
        self.inner.recovery.register(code, handler)
    }

    /// Attempts registered recovery without implicitly reporting an
    /// unhandled error.
    ///
    /// Callers may keep propagating the typed error and report it only if they
    /// become its final responsibility boundary.
    pub fn attempt_recovery(&self, error: Error) -> RecoveryOutcome {
        self.inner.recovery.attempt(error)
    }

    /// Framework-only reporting entry with a bounded, typed origin.
    #[track_caller]
    pub(crate) fn report_with_origin(&self, error: Error, origin: ReportOrigin) -> ReportId {
        let report_site = Location::caller();
        let draft = self.build_draft(&error, origin, report_site);
        let id = self.lock_store().insert(self.inner.runtime_id, draft);

        let report = self.lock_store().report(id).cloned();
        if let Some(report) = report {
            if emit_report(&report, &self.inner.emergency_count) {
                self.lock_store().mark_event_emitted(id);
            }
        }
        id
    }

    fn build_draft(
        &self,
        error: &Error,
        origin: ReportOrigin,
        report_site: &'static Location<'static>,
    ) -> ReportDraft {
        let mut budget = TextBudget::new(MAX_REPORT_BYTES.saturating_sub(RESERVED_FIXED_BYTES));

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

        let backtrace = if should_capture_backtrace(self.inner.config.backtrace, error.severity()) {
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

    fn lock_store(&self) -> std::sync::MutexGuard<'_, ReportStore> {
        self.inner
            .store
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn crash_report_directory(&self) -> Option<&std::path::Path> {
        self.inner.config.crash_report_directory.as_deref()
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new(DiagnosticsConfig::default())
    }
}

fn should_capture_backtrace(policy: BacktracePolicy, severity: ErrorSeverity) -> bool {
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
        let limit = segment_limit.min(self.remaining);
        let mut text = String::with_capacity(limit.min(input.len()));
        let mut truncated = false;

        for character in input.chars() {
            let character = if character.is_control() {
                ' '
            } else {
                character
            };
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
