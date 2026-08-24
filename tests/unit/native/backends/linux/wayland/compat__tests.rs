// 复用被测 registry Widget 与稳定键类型。
use super::{Callback, CallbackDisposition, CallbackKey, CallbackRegistry, StoredCallback};
// 读取稳定错误分类。
use crate::core::Errc;
// 构造与生产 backend 相同的 bounded pending failure source。
use crate::diagnostics::{PendingFailureQueue, PendingFailureSource};
// 使用协议接口类型验证类型错配和 panic 诊断。
use wayland_client::protocol::wl_surface;
// 生成不依赖真实 Wayland connection 的测试类型身份。
use std::any::TypeId;
// 捕获测试主动制造的 registry mutex poison。
use std::panic::{catch_unwind, AssertUnwindSafe};

// 建立一个可独立观察 failure 的 registry fixture。
fn registry_fixture() -> (CallbackRegistry, PendingFailureSource) {
    // 创建单测试专用 bounded queue。
    let queue = PendingFailureQueue::new();
    // 为 registry 创建唯一 failure source。
    let source = queue.source();
    // 返回共享同一 source 的 registry 与 owner 观察句柄。
    (CallbackRegistry::new(source.clone()), source)
}

// 生成不依赖协议连接的稳定 callback map 键。
fn test_key(sequence: u32) -> CallbackKey {
    // 用测试类型身份和显式序号模拟协议对象键。
    (TypeId::of::<u32>(), sequence)
}

// 健康 registry 必须支持 take 后按同一规范形状回插。
#[test]
fn healthy_registry_preserves_owner_across_reinsert() {
    // 构造健康 registry 和本测试不需要读取的 failure source。
    let (registry, _source) = registry_fixture();
    // 建立可观察的稳定测试键。
    let key = test_key(1);
    // 初次 registration 必须接管 owner。
    assert!(registry.insert(key, Box::new(7_u32), "registration"));
    // dispatch take 必须转移同一 owner。
    let callback = registry
        // 从健康 registry 取出唯一值。
        .remove(&key, "dispatch take")
        // 初次登记后必须存在。
        .expect("registered callback owner must be present");
    // callback reinsert 必须重新接管同一 owner。
    assert!(registry.insert(key, callback, "callback reinsert"));
    // 再次取出验证 owner 没有在回插中丢失。
    let callback = registry
        // 从健康 registry 取出回插后的唯一值。
        .remove(&key, "dispatch take")
        // 回插后必须仍然存在。
        .expect("reinserted callback owner must be present");
    // 恢复测试值的具体类型。
    let value = callback
        // 测试 owner 应保持原始 u32 类型。
        .downcast::<u32>()
        // 健康回插不得改变擦除值类型。
        .expect("callback owner type must remain stable");
    // 验证同一 owner 的可观察值未改变。
    assert_eq!(*value, 7);
}

// 持久 callback 的类型恢复与回插必须复用同一外层 owner 分配。
#[test]
fn persistent_callback_reuses_stored_owner_allocation() {
    // 构造健康 registry 与稳定测试键。
    let (registry, _source) = registry_fixture();
    let key = test_key(2);
    // 建立规范的持久 wl_surface callback owner。
    let callback: Callback<wl_surface::WlSurface> =
        Box::new(|_proxy, _event, _qh| CallbackDisposition::Keep);
    let stored: StoredCallback = Box::new(callback);
    // 记录 registry 实际拥有的外层 Box 地址。
    let owner_address = std::ptr::from_ref(stored.as_ref()).cast::<()>();
    assert!(registry.insert(key, stored, "registration"));

    // dispatch take 与强类型恢复不得替换外层 owner。
    let stored = registry
        .remove(&key, "dispatch take")
        .expect("registered callback owner must be present");
    let callback_owner = registry
        .downcast_callback::<wl_surface::WlSurface>(stored)
        .expect("registered callback type must match");
    assert_eq!(
        std::ptr::from_ref(callback_owner.as_ref()).cast::<()>(),
        owner_address
    );

    // 强类型 owner 擦除回插后仍应保持同一分配地址与类型。
    assert!(registry.insert(key, callback_owner, "callback reinsert"));
    let stored = registry
        .remove(&key, "dispatch take")
        .expect("reinserted callback owner must be present");
    assert_eq!(
        std::ptr::from_ref(stored.as_ref()).cast::<()>(),
        owner_address
    );
    assert!(registry
        .downcast_callback::<wl_surface::WlSurface>(stored)
        .is_some());
}

// registry 锁中毒必须为每个生命周期阶段产生稳定 typed failure。
#[test]
fn poisoned_registry_reports_each_lifecycle_operation() {
    // 构造待中毒 registry 与 owner-thread failure 观察句柄。
    let (registry, source) = registry_fixture();
    // 在同线程持锁 panic，避免 StoredCallback 的非 Send 约束影响测试。
    let poison = catch_unwind(AssertUnwindSafe(|| {
        // 取得健康 registry 的唯一 map guard。
        let _guard = registry
            // 仅测试模块直接访问 Widget 内部 mutex。
            .callbacks
            // 初次锁定必须成功。
            .lock()
            // fixture 刚创建，不应已经中毒。
            .expect("fresh callback registry must lock");
        // 主动制造标准 Mutex poison 状态。
        panic!("poison Wayland callback registry for test");
    }));
    // 测试必须成功捕获主动 panic。
    assert!(poison.is_err());
    // registration 失败不得恢复访问中毒 map。
    assert!(!registry.insert(test_key(2), Box::new(1_u32), "registration"));
    // unregistration 失败不得恢复访问中毒 map。
    assert!(registry.remove(&test_key(2), "unregistration").is_none());
    // dispatch take 失败不得恢复访问中毒 map。
    assert!(registry.remove(&test_key(2), "dispatch take").is_none());
    // callback reinsert 失败不得恢复访问中毒 map。
    assert!(!registry.insert(test_key(2), Box::new(1_u32), "callback reinsert"));
    // 按调用顺序验证每个生命周期阶段都进入同一 source。
    for operation in [
        // 初次 callback 登记阶段。
        "registration",
        // 主动 callback 注销阶段。
        "unregistration",
        // dispatch 取出 callback 阶段。
        "dispatch take",
        // callback 执行完成后的回插阶段。
        "callback reinsert",
    ] {
        // owner thread 按 FIFO 取出下一条 registry failure。
        let error = source
            // 读取同一 registry source 的下一条失败。
            .take()
            // 每个损坏操作都必须产生一条失败。
            .expect("poisoned registry operation must enqueue failure");
        // registry mutex poison 必须稳定分类为 InvalidState。
        assert_eq!(error.code(), Errc::InvalidState);
        // 诊断必须保留精确注册生命周期阶段。
        assert!(error.what().contains(operation));
    }
    // 四个操作之后不得生成额外重复 failure。
    assert!(source.take().is_none());
}

// 擦除 callback 类型错配不得继续伪装为未登记 callback。
#[test]
fn callback_type_mismatch_is_reported_to_owner_thread() {
    // 构造健康 registry 与 owner-thread failure 观察句柄。
    let (registry, source) = registry_fixture();
    // 用错误的 u32 owner 模拟同键 callback 类型漂移。
    let callback = registry.downcast_callback::<wl_surface::WlSurface>(Box::new(9_u32));
    // 损坏 owner 必须被隔离而不是交付执行。
    assert!(callback.is_none());
    // owner thread 必须观察到类型错配失败。
    let error = source
        // 读取 registry 产生的唯一失败。
        .take()
        // 类型错配不能静默消失。
        .expect("callback type mismatch must enqueue failure");
    // registry 不变量损坏必须稳定分类为 InvalidState。
    assert_eq!(error.code(), Errc::InvalidState);
    // 诊断必须保留 Wayland 接口与错配阶段。
    assert!(
        // 读取稳定诊断文本。
        error
            // 借用 typed Error 的公开消息。
            .what()
            // 匹配协议接口与 registry 错配阶段。
            .contains("wl_surface callback registry type mismatch")
    );
}

// callback panic 转换必须保持既有 PlatformError 分类。
#[test]
fn callback_panic_is_reported_to_owner_thread() {
    // 构造健康 registry 与 owner-thread failure 观察句柄。
    let (registry, source) = registry_fixture();
    // 模拟 dispatch catch_unwind 已捕获 wl_surface callback panic。
    registry.report_callback_panic::<wl_surface::WlSurface>();
    // owner thread 必须观察到 panic 转换后的唯一失败。
    let error = source
        // 读取 registry 产生的 panic failure。
        .take()
        // callback panic 不能静默消失。
        .expect("callback panic must enqueue failure");
    // callback panic 必须继续分类为 PlatformError。
    assert_eq!(error.code(), Errc::PlatformError);
    // 诊断必须保留 Wayland 接口与 panic 阶段。
    assert!(error.what().contains("wl_surface callback panicked"));
}
