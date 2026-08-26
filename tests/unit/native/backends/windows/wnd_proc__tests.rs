use super::run_wnd_proc_boundary;
use std::cell::Cell;

#[test]
fn wnd_proc_boundary_returns_safe_fallback_after_panic() {
    // 与 crash hook 测试共享全局 panic hook 窗口锁：本测试故意 panic，
    // 若与 crash 测试并行会被其全局 hook 捕获并污染崩溃目录。
    let _lock = crate::diagnostics::PANIC_HOOK_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let notified = Cell::new(false);
    let result = run_wnd_proc_boundary(
        || -> isize { panic!("test wnd_proc ABI panic") },
        || notified.set(true),
        || 0x5a,
    );

    assert_eq!(result, 0x5a);
    assert!(notified.get());
}
