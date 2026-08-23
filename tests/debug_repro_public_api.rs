use std::sync::atomic::{AtomicU64, Ordering};

use uix::core::{Errc, Error};
use uix::diagnostics::{Diagnostics, DiagnosticsConfig};

static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn temp_dir() -> std::path::PathBuf {
    let sequence = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "uix-debug-repro-public-{}-{sequence}",
        std::process::id()
    ))
}

#[test]
fn explicit_manifest_is_atomic_bounded_and_redacted() {
    let directory = temp_dir();
    let _ = std::fs::remove_dir_all(&directory);
    let diagnostics = Diagnostics::new(DiagnosticsConfig::default().debug_mode(true));
    diagnostics.report(
        Error::new(Errc::PlatformError, "password=hunter2")
            .with_source(Error::new(Errc::IoError, "/home/private/token.txt")),
    );
    // 只通过公开开关填满事件环，验证容量淘汰不会增长文件或泄漏任意文本。
    for index in 0..140 {
        diagnostics.set_debug_mode(index % 2 == 1);
    }

    let path = diagnostics
        .write_debug_repro_manifest(&directory)
        .expect("explicit repro manifest should be written");
    let content = std::fs::read_to_string(&path).expect("manifest should be readable");

    assert!(
        path.file_name()
            .is_some_and(|name| { name.to_string_lossy().starts_with("uix-repro-") })
    );
    assert!(content.starts_with("uix-debug-repro\n"));
    assert!(content.contains("reason=manual\n"));
    assert!(content.contains("debug_enabled=true\n"));
    assert!(content.contains("dropped_events=12\n"));
    assert!(content.contains("event_count_retained=128\n"));
    assert!(content.contains("event_count_rendered=128\n"));
    assert!(content.contains("code:platform_error"));
    assert!(content.contains("cause_codes:io_error"));
    assert!(content.len() <= 32 * 1024);
    assert!(!content.contains("hunter2"));
    assert!(!content.contains("/home/private"));
    assert!(!content.contains("password"));
    assert!(!content.contains("token.txt"));

    let temporary_files = std::fs::read_dir(&directory)
        .expect("manifest directory should exist")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(temporary_files, 0);
    let _ = std::fs::remove_dir_all(&directory);
}
