//! E-04 全局反馈门面 `ui::message()` / `ui::notify()` 行为契约。
//!
//! 覆盖：组件构造自动注册、未注册 no-op、显式 register/unregister。

use std::sync::{Mutex, OnceLock};

use uix::prelude::*;
use uix::ui::{register_feedback, unregister_feedback};

/// 门面是进程级注册表：测试串行化避免并行注册互相覆盖。
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

#[test]
fn message_component_registers_facade_automatically() {
    let _guard = serial();
    let _message = Message::new();
    let facade = message();
    assert!(facade.is_available());
    let id = facade.success("已保存");
    assert!(id > 0, "已注册门面应返回稳定 ID");
    let id2 = facade.info("后台任务完成");
    assert!(id2 > id, "ID 单调递增");
}

#[test]
fn notification_component_registers_facade_automatically() {
    let _guard = serial();
    let _notification = Notification::new();
    let facade = notify();
    assert!(facade.is_available());
    let id = facade.info("标题", "描述");
    assert!(id > 0);
    let id2 = facade.warning("标题", "描述");
    assert!(id2 > id);
}

#[test]
fn facade_is_noop_when_unregistered() {
    let _guard = serial();
    unregister_feedback();
    assert!(!message().is_available());
    assert_eq!(message().success("丢失"), 0, "未注册时应 no-op 返回 0");
    assert_eq!(notify().error("t", "d"), 0);
}

#[test]
fn explicit_register_overrides_and_unregister_clears() {
    let _guard = serial();
    let handle = Message::new().handle();
    let notification = Notification::new().handle();
    register_feedback(handle.clone(), notification.clone());

    assert!(message().is_available());
    assert!(notify().is_available());

    unregister_feedback();
    assert!(!message().is_available());
    assert!(!notify().is_available());
}
