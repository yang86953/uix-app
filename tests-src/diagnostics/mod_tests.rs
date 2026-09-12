//! panic hook 测试锁（自 src/diagnostics/mod.rs 移入，零调用探针原样保留待接线）。
//! 经 #[path] 引用，不进发布包。
#![allow(dead_code)]

use super::*;
/// 测试专用：保护进程级全局 panic hook 的安装/恢复窗口。
///
/// panic hook 是进程全局状态。`crash` 的 hook 测试与故意触发 panic 的 ABI
/// 测试（`wnd_proc` / TSF thunk）并行时，后者的 panic 会被前者的全局 hook
/// 捕获并写入其崩溃目录，导致目录断言失败。这些测试共享本锁串行执行。
#[cfg(test)]
pub(crate) static PANIC_HOOK_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
