// 引入被测的私有失败事务 helper。
use super::finish_failed_graphics_candidate;
// 引入 typed error 分类和值。
use crate::core::{Errc, Error};
// 使用单线程 Cell 记录 FnOnce 清理调用次数。
use std::cell::Cell;

// 清理成功必须保留原始初始化错误，并且只调用一次清理动作。
#[test]
fn successful_cleanup_preserves_primary_graphics_error() {
    // 建立可观察的单线程调用计数。
    let calls = Cell::new(0);
    // 构造代表 CPU fallback 初始化失败的主错误。
    let primary = Error::new(Errc::InvalidState, "cpu fallback initialization failed");
    // 执行成功的检查式清理事务。
    let result = finish_failed_graphics_candidate(primary, || {
        // 记录唯一一次清理调用。
        calls.set(calls.get() + 1);
        // 模拟 renderer 已完整释放。
        Ok(())
    });
    // 失败事务不得重复清理同一候选。
    assert_eq!(calls.get(), 1);
    // 清理成功后仍传播原始初始化分类。
    assert_eq!(result.code(), Errc::InvalidState);
    // 清理成功不能伪造额外原因链。
    assert!(result.source_error().is_none());
}

// 清理失败必须成为最终错误，并把原始初始化错误保留为原因。
#[test]
fn failed_cleanup_preserves_both_graphics_errors() {
    // 建立可观察的单线程调用计数。
    let calls = Cell::new(0);
    // 构造触发回滚的原始初始化错误。
    let primary = Error::new(Errc::InvalidState, "cpu fallback initialization failed");
    // 执行失败的检查式清理事务。
    let result = finish_failed_graphics_candidate(primary, || {
        // 记录唯一一次清理调用。
        calls.set(calls.get() + 1);
        // 模拟 backend 仍未完成 teardown。
        Err(Error::new(
            Errc::PlatformError,
            "cpu fallback cleanup failed",
        ))
    });
    // 失败事务同样不得重复清理同一候选。
    assert_eq!(calls.get(), 1);
    // 未完成的 teardown 必须成为最外层错误。
    assert_eq!(result.code(), Errc::PlatformError);
    // 原始初始化失败必须留在可报告原因链中。
    let source = result.source_error().expect("初始化失败原因必须保留");
    // 原始错误分类不能被清理失败覆盖。
    assert_eq!(source.code(), Errc::InvalidState);
}
