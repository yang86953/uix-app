//! Private crash Module owned by the Diagnostics System.
//!
//! The Module owns three narrow responsibilities:
//! - a bounded [`CrashReport`] model with sanitized, size-limited fields;
//! - atomic file writing (temp file + fsync + rename) that never leaves a
//!   partial report behind;
//! - the framework panic hook, which records a crash report when a directory
//!   is configured and always forwards to the previous hook so default
//!   output and termination semantics are preserved (panics are never
//!   swallowed).

use std::any::Any;
use std::cell::Cell;
use std::fmt::Write as _;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::{Errc, Error};

use super::Diagnostics;

/// Bumped when the on-disk crash report layout changes.
pub(crate) const CRASH_SCHEMA_VERSION: u32 = 1;

const MAX_PANIC_MESSAGE_BYTES: usize = 2048;
const MAX_THREAD_BYTES: usize = 256;
const MAX_LOCATION_BYTES: usize = 512;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Bounded, sanitized snapshot of one process panic.
///
/// Every field is capped before rendering; the renderer never receives an
/// unbounded value. No secret-bearing data (environment, arguments, report
/// payloads) is written.
pub(crate) struct CrashReport {
    pub(crate) schema_version: u32,
    pub(crate) runtime_id: u64,
    pub(crate) occurred_at: SystemTime,
    pub(crate) thread: String,
    pub(crate) panic_message: String,
    pub(crate) panic_location: Option<String>,
    pub(crate) retained_reports: usize,
}

/// Captures a panic into the bounded model using the Diagnostics runtime
/// context (runtime id and retained report count).
pub(crate) fn capture(diagnostics: &Diagnostics, info: &PanicHookInfo<'_>) -> CrashReport {
    CrashReport {
        schema_version: CRASH_SCHEMA_VERSION,
        runtime_id: diagnostics.runtime_id(),
        occurred_at: SystemTime::now(),
        thread: truncate_utf8(thread_summary().as_str(), MAX_THREAD_BYTES),
        panic_message: truncate_utf8(&payload_summary(info.payload()), MAX_PANIC_MESSAGE_BYTES),
        panic_location: info.location().map(|location| {
            truncate_utf8(
                &format!(
                    "{}:{}:{}",
                    file_basename(location.file()),
                    location.line(),
                    location.column()
                ),
                MAX_LOCATION_BYTES,
            )
        }),
        retained_reports: diagnostics.snapshot().reports().len(),
    }
}

/// Renders a line-oriented report with sanitized values.
///
/// Control characters (including newlines) are replaced with spaces so the
/// output stays one-field-per-line and cannot be confused with a different
/// report.
pub(crate) fn render(report: &CrashReport) -> String {
    let mut out = String::with_capacity(1024);
    let _ = writeln!(out, "uix-crash-report");
    push_field(
        &mut out,
        "schema_version",
        &report.schema_version.to_string(),
    );
    push_field(&mut out, "runtime_id", &report.runtime_id.to_string());
    let (secs, millis) = unix_components(report.occurred_at);
    push_field(&mut out, "occurred_at_unix_secs", &secs.to_string());
    push_field(&mut out, "occurred_at_unix_millis", &millis.to_string());
    push_field(&mut out, "thread", &report.thread);
    if let Some(location) = &report.panic_location {
        push_field(&mut out, "panic_location", location);
    }
    push_field(&mut out, "panic_message", &report.panic_message);
    push_field(
        &mut out,
        "retained_reports",
        &report.retained_reports.to_string(),
    );
    out
}

fn push_field(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push('=');
    for character in value.chars() {
        if character.is_control() {
            out.push(' ');
        } else {
            out.push(character);
        }
    }
    out.push('\n');
}

/// Writes the report atomically into `directory` and returns the final path.
///
/// The content is written to a unique temp file, fsynced, then renamed into
/// place. A failed write removes the temp file so no partial report is ever
/// left behind. A collision on the final name replaces the previous report.
pub(crate) fn write_atomic(directory: &Path, report: &CrashReport) -> Result<PathBuf, Error> {
    std::fs::create_dir_all(directory).map_err(|error| {
        Error::new(
            Errc::IoError,
            format!(
                "crash report directory unavailable: {}",
                directory.display()
            ),
        )
        .with_source(Error::from(error))
    })?;

    let (secs, millis) = unix_components(report.occurred_at);
    let final_path = directory.join(format!(
        "uix-crash-{secs}-{millis}-{}.txt",
        report.runtime_id
    ));
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let tmp_path = directory.join(format!(
        ".uix-crash-{}-{sequence}-{secs}.tmp",
        std::process::id()
    ));

    let write_result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp_path)?;
        std::io::Write::write_all(&mut file, render(report).as_bytes())?;
        file.sync_all()?;
        drop(file);
        match std::fs::rename(&tmp_path, &final_path) {
            Ok(()) => Ok(()),
            Err(_) => {
                // rename 到已存在目标在部分平台会失败；删除旧报告后重试，
                // 保证同一崩溃不残留两份伪完整文件。
                std::fs::remove_file(&final_path)?;
                std::fs::rename(&tmp_path, &final_path)
            }
        }
    })();

    match write_result {
        Ok(()) => Ok(final_path),
        Err(error) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(Error::new(
                Errc::IoError,
                format!("crash report write failed: {}", final_path.display()),
            )
            .with_source(Error::from(error)))
        }
    }
}

thread_local! {
    static IN_PANIC_HOOK: Cell<bool> = const { Cell::new(false) };
}

struct PanicHookGuard;

impl PanicHookGuard {
    fn enter() -> Option<Self> {
        IN_PANIC_HOOK.with(|running| {
            if running.replace(true) {
                None
            } else {
                Some(Self)
            }
        })
    }
}

impl Drop for PanicHookGuard {
    fn drop(&mut self) {
        IN_PANIC_HOOK.with(|running| running.set(false));
    }
}

/// Installs the framework panic hook for one Diagnostics runtime.
///
/// The hook writes a crash report only when a crash directory is configured;
/// otherwise it behaves exactly like the previous hook. It never swallows a
/// panic: the previous hook is always invoked, so default output and
/// unwind/abort termination semantics are preserved. A panic inside the hook
/// itself (including a second failure while writing) is guarded against
/// recursion and still terminates safely.
pub(crate) fn install_panic_hook(diagnostics: Diagnostics) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let Some(_guard) = PanicHookGuard::enter() else {
            eprintln!("uix: panic occurred inside the panic hook; skipping crash report");
            return;
        };
        if let Some(directory) = diagnostics.crash_report_directory() {
            match write_atomic(directory, &capture(&diagnostics, info)) {
                Ok(path) => eprintln!(
                    "uix: panic captured; crash report written to {}",
                    path.display()
                ),
                Err(error) => eprintln!(
                    "uix: panic hook could not write crash report: {}",
                    error.short_what()
                ),
            }
        }
        previous(info);
    }));
}

fn payload_summary(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        // 非字符串 payload 无法安全还原为可读文本；以固定标记代替，
        // 不把未知内容当作 panic 信息写出。
        "non-string panic payload".to_string()
    }
}

fn thread_summary() -> String {
    let thread = std::thread::current();
    match thread.name() {
        Some(name) => format!("{name}:{:?}", thread.id()),
        None => format!("{:?}", thread.id()),
    }
}

fn file_basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn truncate_utf8(input: &str, limit: usize) -> String {
    if input.len() <= limit {
        return input.to_string();
    }
    let mut end = limit;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    input[..end].to_string()
}

fn unix_components(time: SystemTime) -> (u64, u64) {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (duration.as_secs(), u64::from(duration.subsec_millis())),
        Err(_) => (0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{Diagnostics, DiagnosticsConfig};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    fn panic_hook_test_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::diagnostics::PANIC_HOOK_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn temp_dir() -> PathBuf {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("uix-crash-test-{}-{sequence}", std::process::id()))
    }

    fn sample_report() -> CrashReport {
        CrashReport {
            schema_version: CRASH_SCHEMA_VERSION,
            runtime_id: 7,
            occurred_at: UNIX_EPOCH + std::time::Duration::from_secs(1_754_000_000),
            thread: "main".to_string(),
            panic_message: "boom at \u{7}\nwith control".to_string(),
            panic_location: Some("demo.rs:12:5".to_string()),
            retained_reports: 3,
        }
    }

    #[test]
    fn render_is_line_oriented_and_sanitizes_control_characters() {
        let rendered = render(&sample_report());
        assert!(rendered.starts_with("uix-crash-report\n"));
        assert!(rendered.contains("schema_version=1\n"));
        assert!(rendered.contains("runtime_id=7\n"));
        assert!(rendered.contains("occurred_at_unix_secs=1754000000\n"));
        assert!(rendered.contains("panic_location=demo.rs:12:5\n"));
        assert!(rendered.contains("retained_reports=3\n"));
        // 控制字符（含换行）被替换，行式格式不会被注入。
        assert!(!rendered.contains('\u{7}'));
        assert!(!rendered.contains("\nwith control"));
        assert!(rendered.contains("panic_message=boom at   with control\n"));
        // 每行只有一个 '='（首行 header 除外）且没有空行。
        for (index, line) in rendered.lines().enumerate() {
            assert!(!line.is_empty());
            let expected = if index == 0 { 0 } else { 1 };
            assert_eq!(line.matches('=').count(), expected, "line: {line}");
        }
    }

    #[test]
    fn capture_truncates_unbounded_fields() {
        let diagnostics = Diagnostics::new(DiagnosticsConfig::default());
        let huge = "x".repeat(10_000);
        let report = CrashReport {
            schema_version: CRASH_SCHEMA_VERSION,
            runtime_id: diagnostics.runtime_id(),
            occurred_at: SystemTime::now(),
            thread: truncate_utf8(&huge, MAX_THREAD_BYTES),
            panic_message: truncate_utf8(&huge, MAX_PANIC_MESSAGE_BYTES),
            panic_location: None,
            retained_reports: 0,
        };
        assert!(report.thread.len() <= MAX_THREAD_BYTES);
        assert!(report.panic_message.len() <= MAX_PANIC_MESSAGE_BYTES);
        assert!(render(&report).len() < 4096);
    }

    #[test]
    fn atomic_write_succeeds_leaves_no_temp_and_replaces_same_name() {
        let directory = temp_dir();
        let _ = std::fs::remove_dir_all(&directory);

        let first = write_atomic(&directory, &sample_report());
        assert!(first.is_ok());
        let path = first.unwrap_or_default();
        let first_content = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(first_content.contains("runtime_id=7\n"));

        // 同一崩溃点再次写入：rename 冲突走 replace 分支，内容为最新版本。
        let second = write_atomic(&directory, &sample_report());
        assert!(second.is_ok());
        let second_path = second.unwrap_or_default();
        assert_eq!(path, second_path);

        // 不残留任何临时文件，目录内只有最终报告。
        let entries = std::fs::read_dir(&directory)
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.file_name().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert_eq!(entries.len(), 1, "unexpected entries: {entries:?}");
        assert!(entries[0].ends_with(".txt"));

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn atomic_write_failure_returns_typed_error_and_cleans_temp() {
        let parent = temp_dir();
        let _ = std::fs::remove_dir_all(&parent);
        let _ = std::fs::create_dir_all(&parent);
        // 目录位置被普通文件占据：create_dir_all 必须失败。
        let file_as_directory = parent.join("blocked");
        let _ = std::fs::write(&file_as_directory, b"not a directory");

        let result = write_atomic(&file_as_directory, &sample_report());
        let Err(error) = result else {
            panic!("write_atomic to a blocked path must fail");
        };
        assert_eq!(error.code(), Errc::IoError);
        assert!(error.source_error().is_some());

        // 没有留下任何文件或临时残留。
        let entries = std::fs::read_dir(&parent)
            .map(|entries| entries.count())
            .unwrap_or(0);
        assert_eq!(entries, 1, "only the blocking file may remain");

        let _ = std::fs::remove_dir_all(&parent);
    }

    #[test]
    fn payload_summary_handles_string_and_non_string_payloads() {
        assert_eq!(payload_summary(&"str payload"), "str payload");
        assert_eq!(
            payload_summary(&String::from("owned payload")),
            "owned payload"
        );
        assert_eq!(payload_summary(&42_u32), "non-string panic payload");
    }

    #[test]
    fn panic_hook_writes_crash_report_and_does_not_swallow_panic() {
        let _lock = panic_hook_test_lock();
        let previous = std::panic::take_hook();
        let directory = temp_dir();
        let _ = std::fs::remove_dir_all(&directory);

        let diagnostics =
            Diagnostics::new(DiagnosticsConfig::default().crash_report_directory(&directory));
        diagnostics.install_panic_hook();

        let caught = catch_unwind(AssertUnwindSafe(|| panic!("s3 crash probe")));
        assert!(caught.is_err(), "panic must not be swallowed");

        // 恢复全局 hook 后再读文件，避免断言 panic 再次触发 hook。
        let ours = std::panic::take_hook();
        drop(ours);
        std::panic::set_hook(previous);

        let entries = std::fs::read_dir(&directory)
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.file_name().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert_eq!(entries.len(), 1, "unexpected entries: {entries:?}");
        let report_path = directory.join(&entries[0]);
        let content = std::fs::read_to_string(&report_path).unwrap_or_default();
        assert!(content.contains("panic_message=s3 crash probe"));
        assert!(content.contains("schema_version=1"));

        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn panic_hook_write_failure_still_does_not_swallow_panic() {
        let _lock = panic_hook_test_lock();
        let previous = std::panic::take_hook();
        let parent = temp_dir();
        let _ = std::fs::remove_dir_all(&parent);
        let _ = std::fs::create_dir_all(&parent);
        // 配置的"目录"是普通文件：hook 写入必然失败。
        let file_as_directory = parent.join("blocked");
        let _ = std::fs::write(&file_as_directory, b"not a directory");

        let diagnostics = Diagnostics::new(
            DiagnosticsConfig::default().crash_report_directory(&file_as_directory),
        );
        diagnostics.install_panic_hook();

        let caught = catch_unwind(AssertUnwindSafe(|| panic!("s3 write-failure probe")));
        assert!(
            caught.is_err(),
            "panic must not be swallowed by hook failure"
        );

        let ours = std::panic::take_hook();
        drop(ours);
        std::panic::set_hook(previous);

        let _ = std::fs::remove_dir_all(&parent);
    }
}
