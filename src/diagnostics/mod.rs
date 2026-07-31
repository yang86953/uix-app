//! Runtime-scoped error observation shared by UIX and its applications.
//!
//! SMC boundary for this vertical slice:
//! - `Diagnostics` is the public platform System and owns runtime state,
//!   cross-module orchestration, and failure/reporting policy.
//! - `reporting` and `recovery` are private Modules owned by one System
//!   instance; neither Module is exported or discovers the other.
//! - report draft construction, bounded storage, and tracing emission are
//!   narrow Components selected and owned by the reporting Module.

mod config;
mod recovery;
mod report;
mod reporting;

use std::panic::Location;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::core::Error;

pub use config::{BacktracePolicy, DiagnosticsConfig};
pub use recovery::{RecoveryAction, RecoveryOutcome, RecoverySubscription};
pub(crate) use report::ReportOrigin;
pub use report::{DiagnosticsSnapshot, ErrorReport, ReportId};
use reporting::ReportingModule;

static NEXT_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);

struct DiagnosticsInner {
    runtime_id: u64,
    config: DiagnosticsConfig,
    reporting: ReportingModule,
    recovery: Arc<recovery::RecoveryModule>,
}

/// Public Diagnostics System handle for one runtime.
///
/// The handle exposes only the System contract. Its reporting and recovery
/// Modules, their Components, and their mutable state remain private to this
/// runtime instance.
#[derive(Clone)]
pub struct Diagnostics {
    inner: Arc<DiagnosticsInner>,
}

impl Diagnostics {
    pub fn new(config: DiagnosticsConfig) -> Self {
        let runtime_id = NEXT_RUNTIME_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: Arc::new(DiagnosticsInner {
                runtime_id,
                reporting: ReportingModule::new(&config),
                config,
                recovery: Arc::new(recovery::RecoveryModule::new()),
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
        self.inner.reporting.snapshot()
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
        self.inner.reporting.report(
            self.inner.runtime_id,
            self.inner.config.backtrace,
            error,
            origin,
            report_site,
        )
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
