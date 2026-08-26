use super::{TEST_PANIC_NEXT_CALLBACK, TsfEventSink, TsfTextStore};
use crate::core::{Errc, WindowId};
use crate::diagnostics::PendingFailureQueue;
use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{E_FAIL, HWND};
use windows::Win32::UI::TextServices::{
    ITextStoreACP, ITfCompositionView, ITfContextOwnerCompositionSink,
};

#[test]
fn generated_tsf_thunk_converts_panic_to_hresult_and_owner_failure() {
    // 与 crash hook 测试共享全局 panic hook 窗口锁：本测试故意 panic，
    // 若与 crash 测试并行会被其全局 hook 捕获并污染崩溃目录。
    let _lock = crate::diagnostics::PANIC_HOOK_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let queue = PendingFailureQueue::new();
    let source = queue.source();
    let sink = TsfEventSink {
        events: Arc::new(Mutex::new(VecDeque::new())),
        window_id: WindowId::new(1),
        hwnd: HWND(1usize as *mut std::ffi::c_void),
        pending_failures: source.clone(),
    };
    let (store, _state) = TsfTextStore::create(sink);
    let acp: ITextStoreACP = store.to_interface();

    TEST_PANIC_NEXT_CALLBACK.store(true, Ordering::SeqCst);
    // SAFETY: acp 为 store.to_interface() 刚创建的 COM 接口实例，测试中调用合法。
    let result = unsafe { acp.GetStatus() };
    let Err(error) = result else {
        panic!("a generated TSF thunk panic must become E_FAIL");
    };

    assert_eq!(error.code(), E_FAIL);
    let Some(failure) = source.take() else {
        panic!("a TSF ABI panic must reach the owner failure source");
    };
    assert_eq!(failure.code(), Errc::PlatformError);
    assert!(failure.message().contains("ITextStoreACP::GetStatus"));
    assert!(source.take().is_none());
}

// 第二个生成接口也必须在真实 COM vtable 边界捕获 panic。
#[test]
fn generated_tsf_composition_thunk_converts_panic_to_hresult_and_owner_failure() {
    // 与其它故意 panic 的边界测试串行使用全局 panic hook。
    let _lock = crate::diagnostics::PANIC_HOOK_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // 创建固定容量 owner failure queue。
    let queue = PendingFailureQueue::new();
    // 为当前 TSF store 创建隔离 failure source。
    let source = queue.source();
    // 构造 composition sink 与 owner queue 共享的事件入口。
    let sink = TsfEventSink {
        // 本测试不消费 UI 事件，但保留真实共享队列形状。
        events: Arc::new(Mutex::new(VecDeque::new())),
        // 使用稳定非零窗口标识。
        window_id: WindowId::new(1),
        // 使用测试 HWND，panic 会在正文读取前触发。
        hwnd: HWND(1usize as *mut std::ffi::c_void),
        // 将 ABI failure 投递到本测试隔离 source。
        pending_failures: source.clone(),
    };
    // 创建由 windows-rs implement 宏生成双接口 vtable 的 COM 对象。
    let (store, _state) = TsfTextStore::create(sink);
    // 查询第二个生成接口，确保调用实际穿过 composition vtable。
    let composition: ITfContextOwnerCompositionSink = store.to_interface();

    // 请求下一次生成 thunk 正文在 ffi_guard 内故意 panic。
    TEST_PANIC_NEXT_CALLBACK.store(true, Ordering::SeqCst);
    // 空 composition 参数是 ABI 可表示的测试占位，guard 会在读取前触发。
    // SAFETY: composition 为 to_interface() 刚创建的 COM 接口实例；None 参数在 guard 读取前即触发 panic，无悬垂访问。
    let result = unsafe { composition.OnStartComposition(None::<&ITfCompositionView>) };
    // 生成 thunk 必须把 panic 转成 HRESULT 错误。
    let Err(error) = result else {
        // 成功意味着 panic 越过或绕过了 composition ABI guard。
        panic!("a generated TSF composition thunk panic must become E_FAIL");
    };

    // COM 调用方只能观察到稳定的 E_FAIL。
    assert_eq!(error.code(), E_FAIL);
    // owner thread 必须能够从隔离 source 取回对应 typed failure。
    let Some(failure) = source.take() else {
        // 只返回 HRESULT 而未入队会丢失 owner-thread 诊断责任。
        panic!("a TSF composition ABI panic must reach the owner failure source");
    };
    // 入队失败必须保持平台错误类别。
    assert_eq!(failure.code(), Errc::PlatformError);
    // 消息必须标识第二个接口的具体生成方法。
    assert!(
        failure
            .message()
            .contains("ITfContextOwnerCompositionSink::OnStartComposition")
    );
    // 单次 panic 只能生成一个 owner failure。
    assert!(source.take().is_none());
}
