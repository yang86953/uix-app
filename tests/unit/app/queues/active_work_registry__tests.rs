// 复用活动工作类型与 registry 实现。
use super::*;

// 验证两类 timer 各自受预算约束，同时非回调维护工作不被积压阻塞。
#[test]
// 执行批量到期工作跨轮保留场景。
fn due_timer_budget_preserves_remainder_for_the_next_turn() {
    // 使用同一时刻构造全部到期登记。
    let now = Instant::now();
    // 创建一个窗口独占的活动工作注册表。
    let mut registry = ActiveWorkRegistry::new();
    // 登记不应被 timer 预算阻塞的图形维护工作。
    registry.register(ActiveWorkKind::GraphicsMaintenance, now);
    // 为两种 timer 各登记三个到期回调。
    for id in 1..=3 {
        // 登记 Widget timer 到期事实。
        registry.register(ActiveWorkKind::Timer(id), now);
        // 登记 App timer 到期事实。
        registry.register(ActiveWorkKind::AppTimer(id), now);
    }
    // 复用输出缓冲保存本轮取得执行资格的工作。
    let mut due = Vec::new();

    // 第一轮为每种 timer 只提供两个回调预算。
    registry.drain_due_into_with_budget(now, &mut due, 2);
    // 非回调维护工作必须在第一轮被正常消费。
    assert!(due.contains(&ActiveWorkKind::GraphicsMaintenance));
    // 第一轮只允许两个 Widget timer 回调离开 registry。
    assert_eq!(
        due.iter()
            .filter(|work| matches!(work, ActiveWorkKind::Timer(_)))
            .count(),
        2
    );
    // 第一轮只允许两个 App timer 回调离开 registry。
    assert_eq!(
        due.iter()
            .filter(|work| matches!(work, ActiveWorkKind::AppTimer(_)))
            .count(),
        2
    );
    // 两类 timer 的剩余项保持到期 deadline，使下一轮无需外部新 wake。
    assert_eq!(registry.next_deadline(), Some(now));

    // 第二轮继续取得上一轮保留的工作。
    registry.drain_due_into_with_budget(now, &mut due, 2);
    // 第二轮恰好取得一个剩余 Widget timer。
    assert_eq!(
        due.iter()
            .filter(|work| matches!(work, ActiveWorkKind::Timer(_)))
            .count(),
        1
    );
    // 第二轮恰好取得一个剩余 App timer。
    assert_eq!(
        due.iter()
            .filter(|work| matches!(work, ActiveWorkKind::AppTimer(_)))
            .count(),
        1
    );
    // 全部到期工作被两轮精确消费后 registry 为空。
    assert!(registry.is_empty());
    // 到期项移除后缓存不得继续暴露旧 deadline。
    assert_eq!(registry.next_deadline(), None);
}

#[test]
// 验证动画源完成后同步会移除其 deadline，使窗口重新具备深度休眠条件。
fn animated_source_sync_removes_completed_registration() {
    let now = Instant::now();
    let source = NodeId::new(7);
    let mut registry = ActiveWorkRegistry::new();

    registry.sync_animated_sources([(source, Some(now))]);
    assert!(registry.manages_animation(source));
    assert_eq!(registry.next_deadline(), Some(now));

    registry.sync_animated_sources(std::iter::empty());
    assert!(!registry.manages_animation(source));
    assert!(registry.is_empty());
    assert_eq!(registry.next_deadline(), None);
}

#[test]
// 验证组件逐帧活动只在 active 期间保持 open 登记，结束后恢复来源 deadline 并最终注销。
fn open_widget_animation_only_stays_registered_while_active() {
    let now = Instant::now();
    let source = NodeId::new(9);
    let mut registry = ActiveWorkRegistry::new();

    registry.sync_animated_sources([(source, Some(now))]);
    registry.sync_widget_animations([source]);
    assert_eq!(registry.animation_ids().collect::<Vec<_>>(), vec![source]);
    assert_eq!(registry.next_deadline(), None);

    registry.sync_widget_animations(std::iter::empty());
    assert!(registry.animation_ids().next().is_none());
    assert_eq!(registry.next_deadline(), Some(now));

    registry.sync_animated_sources(std::iter::empty());
    assert!(registry.is_empty());
}

#[test]
fn next_deadline_cache_reuses_and_invalidates_on_actual_changes() {
    let now = Instant::now();
    let later = now + Duration::from_secs(5);
    let mut registry = ActiveWorkRegistry::new();

    // 空表结果也应缓存，避免空闲循环重复扫描。
    assert_eq!(registry.next_deadline(), None);
    assert_eq!(registry.next_deadline_cache.get(), Some(None));

    // 新增与移除更早项必须失效并恢复正确最小值。
    registry.register(ActiveWorkKind::GraphicsMaintenance, later);
    assert_eq!(registry.next_deadline_cache.get(), None);
    assert_eq!(registry.next_deadline(), Some(later));
    registry.register(ActiveWorkKind::Timer(1), now);
    assert_eq!(registry.next_deadline(), Some(now));
    assert!(registry.unregister(ActiveWorkKind::Timer(1)));
    assert_eq!(registry.next_deadline(), Some(later));

    // 写回完全相同的 deadline 不应破坏已计算缓存。
    registry.register(ActiveWorkKind::GraphicsMaintenance, later);
    assert_eq!(registry.next_deadline_cache.get(), Some(Some(later)));

    // open 登记替换有限 deadline 后，最早时间应变为空。
    registry.register_open(ActiveWorkKind::GraphicsMaintenance);
    assert_eq!(registry.next_deadline(), None);

    // App timer 同步只在实际新增、更新或移除时失效。
    registry.sync_app_timers([(7, later)]);
    assert_eq!(registry.next_deadline(), Some(later));
    registry.sync_app_timers([(7, later)]);
    assert_eq!(registry.next_deadline_cache.get(), Some(Some(later)));
    registry.sync_app_timers(std::iter::empty());
    assert_eq!(registry.next_deadline(), None);
}
