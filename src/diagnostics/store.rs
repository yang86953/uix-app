use std::collections::VecDeque;

use super::{DiagnosticsSnapshot, ErrorReport, ReportDraft, ReportId};

pub(crate) struct ReportStore {
    capacity: usize,
    next_id: u64,
    total_reports: u64,
    evicted_reports: u64,
    reports: VecDeque<ErrorReport>,
}

impl ReportStore {
    pub(crate) fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            capacity,
            next_id: 1,
            total_reports: 0,
            evicted_reports: 0,
            reports: VecDeque::with_capacity(capacity),
        }
    }

    pub(crate) fn insert(&mut self, runtime_id: u64, draft: ReportDraft) -> ReportId {
        let id = ReportId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.total_reports = self.total_reports.saturating_add(1);

        if self.reports.len() == self.capacity {
            self.reports.pop_front();
            self.evicted_reports = self.evicted_reports.saturating_add(1);
        }
        self.reports
            .push_back(ErrorReport::from_draft(id, runtime_id, draft));
        id
    }

    pub(crate) fn report(&self, id: ReportId) -> Option<&ErrorReport> {
        self.reports.iter().find(|report| report.id == id)
    }

    pub(crate) fn mark_event_emitted(&mut self, id: ReportId) {
        if let Some(report) = self.reports.iter_mut().find(|report| report.id == id) {
            report.event_emitted = true;
        }
    }

    pub(crate) fn snapshot(&self) -> DiagnosticsSnapshot {
        DiagnosticsSnapshot {
            reports: self.reports.iter().cloned().collect(),
            total_reports: self.total_reports,
            evicted_reports: self.evicted_reports,
        }
    }
}
