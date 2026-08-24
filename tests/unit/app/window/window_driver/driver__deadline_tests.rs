// 复用窗口驱动与同一 System 边界内的队列类型。
use super::*;

// 验证预算后仍有主线程任务时，窗口会登记即时下一轮。
#[test]
// 执行空闲窗口从无 deadline 到队列即时 deadline 的状态迁移。
fn pending_main_thread_work_sets_immediate_deadline() {
    // 固定本次调度判断使用的时刻。
    let now = Instant::now();
    // 使用不可呈现尺寸避免首帧请求干扰队列 deadline 观测。
    let mut driver = WindowDriver::new(0, 0, false);
    // 创建没有失效工作的窗口组件树。
    let mut tree = WidgetTree::new();
    // 创建空活动工作注册表。
    let mut active_work = ActiveWorkRegistry::new();
    // 创建没有 timer 的应用定时器队列。
    let app_timers = AppTimerQueue::new();
    // 创建目标窗口独占的主线程队列。
    let main_thread_queue = MainThreadQueue::new();
    // 创建没有命令的窗口 Agent 状态。
    let agent_commands = WindowAgentState::new();
    // 当前窗口没有待协调根。
    let pending_root = None;

    // 完全空闲且不可呈现的窗口不应登记 deadline。
    assert_eq!(
        driver.next_deadline(
            now,
            &mut tree,
            &mut active_work,
            &app_timers,
            &main_thread_queue,
            &agent_commands,
            &pending_root,
            false,
        ),
        None
    );
    // 投递一项将在下一窗口轮次执行的主线程任务。
    main_thread_queue.enqueue(|| {});
    // 队列非空必须形成当前时刻的即时 deadline。
    assert_eq!(
        driver.next_deadline(
            now,
            &mut tree,
            &mut active_work,
            &app_timers,
            &main_thread_queue,
            &agent_commands,
            &pending_root,
            false,
        ),
        Some(now)
    );
}
