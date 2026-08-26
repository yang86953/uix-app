// 复用当前 Module 的私有模型。
use super::*;
// 创建测试 runtime-scoped failure source。
use crate::diagnostics::PendingFailureQueue;

// 验证 surface 在多 output 间迁移时 drawable 与 DPR 始终同代。
#[test]
// 执行 enter、动态 Scale 与 leave 的完整状态序列。
fn output_transitions_update_only_the_target_surface() {
    // 创建测试 failure queue。
    let failures = PendingFailureQueue::new();
    // 所有被测 Widget 必须共享同一个可观察 failure source。
    let failure_source = failures.source();
    // 创建 backend 级 output registry。
    let registry = Arc::new(WaylandOutputScaleRegistry::new(failure_source.clone()));
    // 登记 scale=1 的默认 output。
    registry.register_output(10, true);
    // 提交默认 output scale。
    registry.update_output_scale(10, 1);
    // 登记第二个 HiDPI output。
    registry.register_output(20, false);
    // 提交第二个 output scale=2。
    registry.update_output_scale(20, 2);
    // 创建共享事件队列。
    let events = Arc::new(Mutex::new(std::collections::VecDeque::new()));
    // 创建 800x600、初始 scale=1 的目标窗口状态。
    let surface = WaylandWindowScaleState::new(
        // 使用稳定测试窗口身份。
        WindowId::new(7),
        // 初始逻辑宽度。
        800,
        // 初始逻辑高度。
        600,
        // 从 registry 读取 fallback scale。
        registry.preferred_scale(),
        // 注入目标窗口事件队列。
        Arc::clone(&events),
        // 注入同一 runtime failure source。
        failure_source.clone(),
    );
    // 登记逐窗弱订阅。
    registry.register_surface(&surface);

    // 进入默认 output 不改变 scale。
    assert_eq!(surface.enter_output(10, registry.scale_for(10)), None);
    // 同时进入 HiDPI output 时采用最大 scale=2。
    assert_eq!(surface.enter_output(20, registry.scale_for(20)), Some(2));
    // 读取同代 surface 快照。
    let snapshot = surface.metrics().snapshot().expect("读取 scale=2 快照");
    // logical 800x600 必须映射为 drawable 1600x1200。
    assert_eq!(
        (snapshot.logical_width, snapshot.logical_height),
        (800, 600)
    );
    // 物理尺寸精确乘当前 scale。
    assert_eq!(
        (snapshot.drawable_width, snapshot.drawable_height),
        (1600, 1200)
    );
    // DPR 事实为整数 2。
    assert_eq!(snapshot.scale, 2);

    // compositor 动态把当前 output 更新为 scale=3。
    registry.update_output_scale(20, 3);
    // 读取更新后的同代快照。
    let snapshot = surface.metrics().snapshot().expect("读取 scale=3 快照");
    // drawable 随目标 output 更新为 2400x1800。
    assert_eq!(
        (snapshot.drawable_width, snapshot.drawable_height),
        (2400, 1800)
    );
    // scale 同步更新为 3。
    assert_eq!(snapshot.scale, 3);

    // 离开 HiDPI output 后回到默认 output scale=1。
    assert_eq!(
        surface.leave_output(20, registry.preferred_scale()),
        Some(1)
    );
    // 读取返回路径快照。
    let snapshot = surface.metrics().snapshot().expect("读取返回 scale=1 快照");
    // logical extent 保持不变。
    assert_eq!(
        (snapshot.logical_width, snapshot.logical_height),
        (800, 600)
    );
    // drawable 回到 800x600。
    assert_eq!(
        (snapshot.drawable_width, snapshot.drawable_height),
        (800, 600)
    );
    // 有效 scale 回到 1。
    assert_eq!(snapshot.scale, 1);

    // 三次 scale 变化各排队一条目标窗口 logical resize。
    let events = events.lock().expect("读取 scale 事件队列");
    // enter、动态更新、leave 共三条。
    assert_eq!(events.len(), 3);
    // 所有事件都严格路由到窗口 7。
    assert!(
        events
            .iter()
            .all(|event| event.window_id == Some(WindowId::new(7)))
    );
    // 健康状态序列不得产生 pending failure。
    assert!(failure_source.take().is_none());
}

// 验证程序化 resize 同代更新 drawable，并把目标窗口事件交给唯一布局管线。
#[test]
fn programmatic_resize_updates_metrics_and_queues_targeted_event() {
    // 创建可观察 failure source 与逐窗事件队列。
    let failures = PendingFailureQueue::new();
    let failure_source = failures.source();
    let events = Arc::new(Mutex::new(std::collections::VecDeque::new()));
    // 使用 scale=2 证明 logical 与 drawable 尺寸不会混淆。
    let surface = WaylandWindowScaleState::new(
        WindowId::new(9),
        800,
        600,
        2,
        Arc::clone(&events),
        failure_source.clone(),
    );

    surface
        .publish_programmatic_resize(1000, 700)
        .expect("程序化 resize 必须发布成功");

    // surface metrics 必须以同一 scale 发布新 logical/drawable 尺寸。
    let snapshot = surface.metrics().snapshot().expect("读取 resize 快照");
    assert_eq!(
        (snapshot.logical_width, snapshot.logical_height),
        (1000, 700)
    );
    assert_eq!(
        (snapshot.drawable_width, snapshot.drawable_height),
        (2000, 1400)
    );
    // 唯一事件必须携带同一目标窗口与 logical 尺寸。
    let events = events.lock().expect("读取 resize 事件队列");
    assert_eq!(events.len(), 1);
    let event = events.front().expect("必须存在 resize 事件");
    assert_eq!(event.window_id, Some(WindowId::new(9)));
    assert_eq!(
        event.type_,
        crate::platform::windowing::event::UiEventType::WindowResize
    );
    assert_eq!(
        event.payload,
        crate::platform::windowing::event::UiEventPayload::Resize(
            crate::platform::windowing::event::ResizeData {
                width: 1000,
                height: 700,
            }
        )
    );
    assert!(failure_source.take().is_none());
}
