//! `diagnostics/crash.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::super::mod_tests::PANIC_HOOK_TEST_LOCK;
use super::{install_panic_hook, Path, PathBuf};
use crate::diagnostics::{Diagnostics, DiagnosticsConfig};

fn scratch_dir(label: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "uix-crash-hook-test-{}-{label}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&directory);
    directory
}

/// 在持有进程级 hook 串行锁的窗口内安装框架 hook：先登记一个可观察的
/// 先前 hook，结束时恢复测试前的默认 hook。锁内覆盖从登记到恢复的完整
/// 窗口，避免与其它设置全局 hook 的测试交错。
fn with_installed_hook<T>(
    diagnostics: Diagnostics,
    previous: impl Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync + 'static,
    run: impl FnOnce() -> T,
) -> T {
    let _guard = PANIC_HOOK_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(previous));
    install_panic_hook(diagnostics);
    let result = run();
    let _installed = std::panic::take_hook();
    std::panic::set_hook(default_hook);
    result
}

/// panic 必须发生在独立线程：join 返回 `Err` 同时证明 hook 未吞 panic。
fn panic_in_background_thread(message: &'static str) {
    let worker = std::thread::spawn(move || panic!("{}", message));
    assert!(worker.join().is_err(), "hook must not swallow the panic");
}

fn artifact_names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .expect("crash directory must exist")
        .map(|entry| entry.expect("entry readable").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn panic_with_directory_writes_atomic_reports_and_forwards_previous() {
    let directory = scratch_dir("atomic-forward");
    let forwarded = Arc::new(AtomicBool::new(false));
    let previous_flag = Arc::clone(&forwarded);

    let diagnostics = Diagnostics::new(
        DiagnosticsConfig::default().crash_report_directory(&directory),
    );
    with_installed_hook(
        diagnostics,
        move |_| previous_flag.store(true, Ordering::Relaxed),
        || {
            panic_in_background_thread("crash probe\nsecond\tline");
        },
    );

    let names = artifact_names(&directory);
    assert_eq!(names.len(), 2, "exactly one crash and one repro manifest: {names:?}");
    assert!(names.iter().any(|name| name.starts_with("uix-crash-")));
    assert!(names.iter().any(|name| name.starts_with("uix-repro-")));
    // 原子写入契约：不残留 tmp 临时文件或部分报告。
    assert!(
        names.iter().all(|name| !name.starts_with('.')),
        "atomic write must leave no tmp artifacts: {names:?}"
    );

    let crash = names
        .iter()
        .find(|name| name.starts_with("uix-crash-"))
        .expect("crash report present");
    let contents = std::fs::read_to_string(directory.join(crash)).expect("crash readable");
    assert!(contents.starts_with("uix-crash-report\n"));
    let message_line = contents
        .lines()
        .find(|line| line.starts_with("panic_message="))
        .expect("panic message field");
    assert!(message_line.contains("crash probe"), "{message_line}");
    // 行式渲染契约：字段内的换行与制表符被替换为空格。
    assert!(
        !message_line.chars().any(char::is_control),
        "field values must stay on one line: {message_line}"
    );
    assert!(contents.lines().any(|line| line.starts_with("panic_location=")));
    assert!(contents.lines().any(|line| line.starts_with("retained_reports=")));

    let repro = names
        .iter()
        .find(|name| name.starts_with("uix-repro-"))
        .expect("repro manifest present");
    let repro_contents =
        std::fs::read_to_string(directory.join(repro)).expect("repro readable");
    assert!(repro_contents.starts_with("uix-debug-repro\n"));

    assert!(forwarded.load(Ordering::Relaxed), "previous hook must be forwarded");
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn panic_hook_survives_unwritable_directory_and_still_forwards() {
    // 用一个已存在的文件充当目录，迫使 create_dir_all 失败：hook 必须
    // 写失败安全（不再二次 panic），仍转发先前 hook，不吞 panic。
    let blocker = std::env::temp_dir().join(format!(
        "uix-crash-hook-blocker-{}.txt",
        std::process::id()
    ));
    std::fs::write(&blocker, "not a directory").expect("blocker writable");

    let forwarded = Arc::new(AtomicBool::new(false));
    let previous_flag = Arc::clone(&forwarded);

    let diagnostics =
        Diagnostics::new(DiagnosticsConfig::default().crash_report_directory(&blocker));
    with_installed_hook(
        diagnostics,
        move |_| previous_flag.store(true, Ordering::Relaxed),
        || {
            panic_in_background_thread("unwritable probe");
        },
    );

    assert!(forwarded.load(Ordering::Relaxed), "previous hook must be forwarded");
    // 失败清理契约：不允许留下临时文件。
    let siblings: Vec<String> = std::fs::read_dir(blocker.parent().expect("temp exists"))
        .expect("temp readable")
        .map(|entry| entry.expect("entry readable").file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".uix-crash") || name.starts_with(".uix-repro"))
        .collect();
    assert!(siblings.is_empty(), "failed writes must clean tmp files: {siblings:?}");
    let _ = std::fs::remove_file(&blocker);
}
