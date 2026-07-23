use std::path::PathBuf;

/// Controls when a backtrace is captured at the final reporting boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacktracePolicy {
    Disabled,
    FatalOnly,
    ErrorsAndFatal,
}

/// Immutable configuration used to create one runtime-scoped [`Diagnostics`].
///
/// Safety budgets for individual reports are fixed framework invariants and
/// deliberately are not configurable.
#[derive(Debug, Clone)]
pub struct DiagnosticsConfig {
    pub(crate) report_capacity: usize,
    pub(crate) crash_report_directory: Option<PathBuf>,
    pub(crate) backtrace: BacktracePolicy,
}

impl DiagnosticsConfig {
    /// Sets the number of retained reports. A zero capacity is normalized to
    /// one so every successful `report` call can retain its report.
    pub fn report_capacity(mut self, capacity: usize) -> Self {
        self.report_capacity = capacity.max(1);
        self
    }

    /// Sets the directory used by the emergency crash-report path.
    pub fn crash_report_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.crash_report_directory = Some(directory.into());
        self
    }

    /// Sets the final-boundary backtrace capture policy.
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
