// 复用被测私有 helper、HANDLE 与框架错误类型。
use super::*;
// 使用 Cell 记录注入关闭动作的精确调用次数。
use std::cell::Cell;

// 成功关闭必须清空 owner 槽位，后续幂等调用不得重复关闭。
#[test]
fn successful_fence_event_close_runs_once() {
    // 使用不会传给 Win32 的占位 HANDLE 验证所有权事务。
    let mut slot = Some(HANDLE::default());
    // 记录注入关闭动作的调用次数。
    let calls = Cell::new(0_u32);
    // 首次关闭应消费唯一 owner。
    let first = close_owned_fence_event_with(&mut slot, |_| {
        // 记录底层关闭动作确实执行一次。
        calls.set(calls.get() + 1);
        // 模拟 Win32 成功关闭。
        Ok(())
    });
    // 首次关闭必须成功。
    assert!(first.is_ok());
    // 成功后 owner 槽位必须为空。
    assert!(slot.is_none());
    // 再次调用不得触碰底层关闭动作。
    let second = close_owned_fence_event_with(&mut slot, |_| {
        // 若执行到这里就说明幂等关闭破坏了唯一所有权。
        calls.set(calls.get() + 1);
        // 保持闭包返回类型稳定。
        Ok(())
    });
    // 幂等关闭必须继续成功。
    assert!(second.is_ok());
    // 两次入口合计只能执行一次底层关闭。
    assert_eq!(calls.get(), 1);
}

// 关闭失败必须恢复 owner，并允许下一次调用完成关闭。
#[test]
fn failed_fence_event_close_restores_owner_for_retry() {
    // 使用不会传给 Win32 的占位 HANDLE 验证失败恢复。
    let mut slot = Some(HANDLE::default());
    // 记录失败与重试两次底层调用。
    let calls = Cell::new(0_u32);
    // 首次关闭注入稳定的 typed error。
    let first = close_owned_fence_event_with(&mut slot, |_| {
        // 记录第一次底层关闭尝试。
        calls.set(calls.get() + 1);
        // 模拟 Win32 CloseHandle 失败。
        Err(platform_error("injected fence event close failure"))
    });
    // 首次关闭必须向上传播错误。
    let error = first.expect_err("injected close failure must propagate");
    // 错误必须保留平台失败分类。
    assert_eq!(error.code(), Errc::PlatformError);
    // 失败后 HANDLE 必须恢复给同一 owner。
    assert!(slot.is_some());
    // 重试关闭应消费恢复后的 HANDLE。
    let retry = close_owned_fence_event_with(&mut slot, |_| {
        // 记录第二次底层关闭尝试。
        calls.set(calls.get() + 1);
        // 模拟 Drop 或显式 owner 重试成功。
        Ok(())
    });
    // 重试必须成功。
    assert!(retry.is_ok());
    // 重试成功后 owner 槽位必须为空。
    assert!(slot.is_none());
    // 失败与重试合计应精确调用底层两次。
    assert_eq!(calls.get(), 2);
}
