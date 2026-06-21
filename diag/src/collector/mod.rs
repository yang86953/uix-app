// ============================================================================
// uix-diag/src/collector.rs — Thread-safe error collection and aggregation
// ============================================================================

use crate::error::*;
use crate::log::*;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;

// ════════════════════════════════════════════════════════════════════════════
// 收集器配置
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct CollectorConfig {
    pub max_errors: usize,
    pub auto_log: bool,
    pub auto_log_level: Level,
    pub deduplicate: bool,
}

impl Default for CollectorConfig {
    fn default() -> Self {
        Self {
            max_errors: 4096,
            auto_log: true,
            auto_log_level: Level::Error,
            deduplicate: false,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 收集器快照
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct CollectorSnapshot {
    pub timestamp: DateTime<Utc>,
    pub total_collected: usize,
    pub stored_count: usize,
    pub errors: Vec<Error>,
}

impl CollectorSnapshot {
    pub fn count_by_code(&self) -> HashMap<Errc, usize> {
        let mut counts = HashMap::new();
        for err in &self.errors {
            *counts.entry(err.code()).or_insert(0) += 1;
        }
        counts
    }

    pub fn count_by_category(&self) -> HashMap<&'static str, usize> {
        let mut counts = HashMap::new();
        for err in &self.errors {
            *counts.entry(err.code().category()).or_insert(0) += 1;
        }
        counts
    }
}

impl fmt::Display for CollectorSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ts = self.timestamp.format("%H:%M:%S");
        writeln!(f, "Collector snapshot @ {}", ts)?;
        writeln!(f, "  Total collected: {}", self.total_collected)?;
        writeln!(f, "  Stored: {}", self.stored_count)?;
        let by_code = self.count_by_code();
        if !by_code.is_empty() {
            writeln!(f, "  By code:")?;
            for (code, count) in by_code {
                writeln!(f, "    {}: {}", code, count)?;
            }
        }
        for err in &self.errors {
            writeln!(f, "  - {}", err.short_what())?;
        }
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Collector — 错误收集器（线程安全）
// ════════════════════════════════════════════════════════════════════════════

type ErrorCallback = Box<dyn Fn(&Error) + Send + Sync>;

struct CollectorInner {
    errors: VecDeque<Error>,
    config: CollectorConfig,
    callbacks: HashMap<u64, ErrorCallback>,
    next_callback_id: u64,
    last_hash: u64,
}

pub struct Collector {
    inner: RwLock<CollectorInner>,
    total_collected: AtomicUsize,
}

impl Collector {
    pub fn instance() -> &'static Self {
        static COLLECTOR: std::sync::OnceLock<Collector> = std::sync::OnceLock::new();
        COLLECTOR.get_or_init(|| Collector {
            inner: RwLock::new(CollectorInner {
                errors: VecDeque::new(),
                config: CollectorConfig::default(),
                callbacks: HashMap::new(),
                next_callback_id: 1,
                last_hash: 0,
            }),
            total_collected: AtomicUsize::new(0),
        })
    }

    pub fn configure(&self, config: CollectorConfig) {
        self.inner.write().unwrap_or_else(|e| e.into_inner()).config = config;
    }

    pub fn get_config(&self) -> CollectorConfig {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .config
            .clone()
    }

    pub fn set_max_errors(&self, max: usize) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.config.max_errors = max;
        while inner.errors.len() > max {
            inner.errors.pop_front();
        }
    }

    pub fn set_auto_log(&self, enabled: bool, level: Level) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.config.auto_log = enabled;
        inner.config.auto_log_level = level;
    }

    pub fn set_deduplicate(&self, enabled: bool) {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .config
            .deduplicate = enabled;
    }

    pub fn collect(&self, err: Error) -> usize {
        let severity = err.severity();
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());

        // Deduplicate check
        if inner.config.deduplicate {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            err.hash(&mut hasher);
            let h = hasher.finish();
            if h == inner.last_hash {
                if let Some(last) = inner.errors.back() {
                    if last.code() == err.code() && last.message() == err.message() {
                        self.total_collected.fetch_add(1, Ordering::Relaxed);
                        return inner.errors.len();
                    }
                }
            }
            inner.last_hash = h;
        }

        inner.errors.push_back(err);
        while inner.errors.len() > inner.config.max_errors {
            inner.errors.pop_front();
        }

        let stored = inner.errors.len();
        self.total_collected.fetch_add(1, Ordering::Relaxed);

        // Severity-based auto-log & routing
        if inner.config.auto_log || severity >= crate::error::ErrorSeverity::Error {
            let level = match severity {
                crate::error::ErrorSeverity::Info => Level::Info,
                crate::error::ErrorSeverity::Warning => Level::Warn,
                crate::error::ErrorSeverity::Error => Level::Error,
                crate::error::ErrorSeverity::Fatal => Level::Fatal,
            };
            let err_copy = inner.errors.back().cloned();
            drop(inner);
            if let Some(ref e) = err_copy {
                Logger::instance().log_error(e, level);
            }
            if severity.should_abort() {
                crate::fatal::dump_crash_report();
                std::process::abort();
            }
        } else {
            drop(inner);
        }

        stored
    }

    pub fn collect_fn<F>(&self, f: F) -> usize
    where
        F: FnOnce() -> Error,
    {
        self.collect(f())
    }

    pub fn report(&self, code: Errc, message: impl Into<String>) -> usize {
        self.collect(Error::new(code, message))
    }

    pub fn collect_exception(&self, msg: impl Into<String>, code: Errc) -> usize {
        self.collect(Error::new(code, msg))
    }

    pub fn on_collect<F>(&self, callback: F) -> u64
    where
        F: Fn(&Error) + Send + Sync + 'static,
    {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        let id = inner.next_callback_id;
        inner.next_callback_id += 1;
        inner.callbacks.insert(id, Box::new(callback));
        id
    }

    pub fn remove_callback(&self, token: u64) -> bool {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .callbacks
            .remove(&token)
            .is_some()
    }

    pub fn total_collected(&self) -> usize {
        self.total_collected.load(Ordering::Relaxed)
    }

    pub fn stored_count(&self) -> usize {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .errors
            .len()
    }

    pub fn has_errors(&self) -> bool {
        !self
            .inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .errors
            .is_empty()
    }

    pub fn errors(&self) -> Vec<Error> {
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        inner.errors.iter().rev().cloned().collect()
    }

    pub fn errors_by_code(&self, code: Errc) -> Vec<Error> {
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        inner
            .errors
            .iter()
            .rev()
            .filter(|e| e.code() == code)
            .cloned()
            .collect()
    }

    pub fn errors_in_window(&self, since: DateTime<Utc>, until: DateTime<Utc>) -> Vec<Error> {
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        inner
            .errors
            .iter()
            .rev()
            .filter(|e| e.timestamp() >= since && e.timestamp() <= until)
            .cloned()
            .collect()
    }

    pub fn errors_if<F>(&self, predicate: F) -> Vec<Error>
    where
        F: Fn(&Error) -> bool,
    {
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        inner
            .errors
            .iter()
            .rev()
            .filter(|e| predicate(e))
            .cloned()
            .collect()
    }

    pub fn count_by_code(&self) -> HashMap<Errc, usize> {
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        let mut counts = HashMap::new();
        for err in &inner.errors {
            *counts.entry(err.code()).or_insert(0) += 1;
        }
        counts
    }

    pub fn count_by_category(&self) -> HashMap<&'static str, usize> {
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        let mut counts = HashMap::new();
        for err in &inner.errors {
            *counts.entry(err.code().category()).or_insert(0) += 1;
        }
        counts
    }

    pub fn snapshot(&self) -> CollectorSnapshot {
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        CollectorSnapshot {
            timestamp: Utc::now(),
            total_collected: self.total_collected.load(Ordering::Relaxed),
            stored_count: inner.errors.len(),
            errors: inner.errors.iter().rev().cloned().collect(),
        }
    }

    pub fn clear(&self) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.errors.clear();
        inner.last_hash = 0;
    }

    pub fn clear_by_code(&self, code: Errc) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.errors.retain(|e| e.code() != code);
    }

    pub fn clear_before(&self, tp: DateTime<Utc>) {
        let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
        inner.errors.retain(|e| e.timestamp() >= tp);
    }

    pub fn dump(&self) -> String {
        self.snapshot().to_string()
    }

    pub fn summary(&self) -> String {
        let total = self.total_collected.load(Ordering::Relaxed);
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        let stored = inner.errors.len();
        let mut result = format!("{} errors collected ({} stored)", total, stored);
        if !inner.errors.is_empty() {
            let mut top_codes = HashMap::new();
            for err in &inner.errors {
                *top_codes.entry(err.code()).or_insert(0) += 1;
            }
            result.push_str(", top: ");
            let mut sorted: Vec<_> = top_codes.into_iter().collect();
            sorted.sort_by_key(|b| std::cmp::Reverse(b.1));
            for (i, (code, count)) in sorted.iter().take(3).enumerate() {
                if i > 0 {
                    result.push_str(", ");
                }
                result.push_str(&format!("{}={}", code, count));
            }
        }
        result
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ScopedCollector — 作用域收集器 (split to scoped.rs for ≤400 lines)
// ════════════════════════════════════════════════════════════════════════════

mod scoped;
pub use scoped::*;
