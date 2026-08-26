// 仅在公开无窗口测试驱动可用时编译焦点句柄生命周期契约。
#![cfg(feature = "test-harness")]

// 引入公开响应式状态、焦点句柄与视图扩展入口。
use uix::ui::{EventExt, FocusHandle, FocusHandleError, State, button};
// 使用与真实语义路径一致的公开测试应用驱动。
use uix::ui::test_harness::TestApp;

// 验证句柄未绑定时同时满足公开布尔状态和命令错误契约。
fn assert_unbound(handle: &FocusHandle) {
    assert!(!handle.is_bound());
    assert_eq!(handle.focus(), Err(FocusHandleError::Unbound));
}

// 验证同一可重建视图上的句柄复用、替换解绑和清除生命周期。
#[test]
fn focus_handle_rebuild_replacement_and_clear_are_unbound_cleanly() {
    let stage = State::new(0_u8);
    let first = FocusHandle::new();
    let second = FocusHandle::new();

    let root_stage = stage.clone();
    let root_first = first.clone();
    let root_second = second.clone();
    let mut app = TestApp::new((320.0, 200.0), move || match root_stage.get() {
        // 初始节点绑定第一个句柄。
        0 => button("first").focus_handle(&root_first),
        // 状态驱动同一节点重建，但继续复用第一个句柄。
        1 => button("first-updated").focus_handle(&root_first),
        // 同一节点改绑第二个句柄，旧句柄应立即解除绑定。
        2 => button("second").focus_handle(&root_second),
        // 不声明句柄，验证最后一个绑定也会被清除。
        _ => button("plain").into(),
    });

    assert!(first.is_bound());
    assert_unbound(&second);

    // 仅改变状态值触发协调，句柄仍应绑定到同一个重建节点。
    stage.set(1);
    app.settle().expect("同句柄状态重建应稳定");
    assert!(first.is_bound());
    assert_unbound(&second);

    // 替换句柄后，旧句柄必须返回 Unbound，新句柄必须已绑定。
    stage.set(2);
    app.settle().expect("句柄替换应稳定");
    assert_unbound(&first);
    assert!(second.is_bound());

    // 清除节点声明后，第二个句柄也必须返回 Unbound。
    stage.set(3);
    app.settle().expect("句柄清除应稳定");
    assert_unbound(&second);
}
