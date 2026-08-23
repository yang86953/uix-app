use super::*;
use crate::diagnostics::{Diagnostics, DiagnosticsConfig};
use std::panic::{AssertUnwindSafe, catch_unwind};

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
    assert_eq!(entries.len(), 2, "unexpected entries: {entries:?}");
    let crash_name = entries
        .iter()
        .find(|name| name.starts_with("uix-crash-"))
        .expect("panic hook should write crash report");
    let repro_name = entries
        .iter()
        .find(|name| name.starts_with("uix-repro-"))
        .expect("panic hook should write reproduction manifest");
    let report_path = directory.join(crash_name);
    let content = std::fs::read_to_string(&report_path).unwrap_or_default();
    assert!(content.contains("panic_message=s3 crash probe"));
    assert!(content.contains("schema_version=1"));
    let repro = std::fs::read_to_string(directory.join(repro_name)).unwrap_or_default();
    assert!(repro.contains("reason=panic\n"));
    assert!(!repro.contains("s3 crash probe"));

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

    let diagnostics =
        Diagnostics::new(DiagnosticsConfig::default().crash_report_directory(&file_as_directory));
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
