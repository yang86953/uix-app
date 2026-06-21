use crate::collector::Collector;
use crate::error::Error;

// ════════════════════════════════════════════════════════════════════════════
// ScopedCollector — 作用域收集器
// ════════════════════════════════════════════════════════════════════════════

pub struct ScopedCollector {
    collector: &'static Collector,
    pending: Vec<Error>,
}

impl ScopedCollector {
    pub fn new(collector: &'static Collector) -> Self {
        Self {
            collector,
            pending: Vec::new(),
        }
    }

    pub fn collect(&mut self, err: Error) {
        self.pending.push(err);
    }

    pub fn flush(&mut self) {
        for err in self.pending.drain(..) {
            self.collector.collect(err);
        }
    }

    pub fn discard(&mut self) {
        self.pending.clear();
    }
}

impl Drop for ScopedCollector {
    fn drop(&mut self) {
        self.flush();
    }
}

impl Default for ScopedCollector {
    fn default() -> Self {
        Self::new(Collector::instance())
    }
}
