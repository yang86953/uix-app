// 引入逐窗注册表 Widget。
use super::FileDropWindowRegistry;

// 重复启用与禁用必须保持单一布尔事实。
#[test]
fn window_registration_is_idempotent() {
    // 使用整数代替平台 WindowId 验证纯集合语义。
    let mut registry = FileDropWindowRegistry::default();
    // 初始窗口没有接收资格。
    assert!(!registry.contains(&7_u64));
    // 首次启用发布资格。
    registry.insert(7_u64);
    // 重复启用不得改变可观察结果。
    registry.insert(7_u64);
    // 窗口现在可接受拖放。
    assert!(registry.contains(&7_u64));
    // 首次禁用撤销资格。
    registry.remove(&7_u64);
    // 重复禁用保持无资格。
    registry.remove(&7_u64);
    // 最终状态必须为禁用。
    assert!(!registry.contains(&7_u64));
}

// backend shutdown 必须同时撤销所有窗口资格。
#[test]
fn clear_revokes_every_window() {
    // 建立含两个窗口的注册表。
    let mut registry = FileDropWindowRegistry::default();
    // 启用第一窗口。
    registry.insert(1_u64);
    // 启用第二窗口。
    registry.insert(2_u64);
    // shutdown 执行全量撤销。
    registry.clear();
    // 第一窗口不再启用。
    assert!(!registry.contains(&1_u64));
    // 第二窗口也不再启用。
    assert!(!registry.contains(&2_u64));
}
