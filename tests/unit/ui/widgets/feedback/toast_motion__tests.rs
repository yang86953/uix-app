// 引入被测队列与 motion。
use super::*;
// 引入线程安全事实记录。
use std::sync::Mutex;

// 验证内容更新保持计时，只有 duration 更新重启计时。
#[test]
fn declaration_updates_restart_only_changed_duration() {
    // 创建字符串测试队列。
    let queue = ToastQueue::new();
    // 使用稳定外部 ID 写入一秒条目。
    queue.update_external(9, "初始".to_string(), 1_000);
    // 创建空 motion 状态。
    let mut motion = ToastMotion::default();
    // 首次同步创建进入条目。
    assert!(motion.sync(
        &queue,
        AnimationConfig::fade_in(0.01),
        AnimationConfig::fade_out(0.01),
    ));
    // 完成进入动画并开始一秒 Holding 计时。
    motion.update(1.0);
    // 保存初始 timer id 与截止时长。
    let initial_timer = motion.active_timer().expect("一秒条目应有 timer");
    // 只更新内容并保持 duration。
    queue.update_external(9, "内容更新".to_string(), 1_000);
    // 新内容必须触发重绘同步。
    assert!(motion.sync(
        &queue,
        AnimationConfig::fade_in(0.01),
        AnimationConfig::fade_out(0.01),
    ));
    // 内容更新不得重启或更换 timer。
    assert_eq!(motion.active_timer(), Some(initial_timer));
    // 更新为两秒 duration。
    queue.update_external(9, "内容更新".to_string(), 2_000);
    // duration 更新必须同步。
    assert!(motion.sync(
        &queue,
        AnimationConfig::fade_in(0.01),
        AnimationConfig::fade_out(0.01),
    ));
    // 新 timer 必须从两秒重新开始且更换 id。
    let restarted = motion.active_timer().expect("两秒条目应有 timer");
    // timer id 变化使旧计时事件失效。
    assert_ne!(restarted.0, initial_timer.0);
    // 新截止时长准确为两秒。
    assert_eq!(restarted.1, Duration::from_secs(2));
}

// 验证关闭原因只发布一次且释放不发布关闭事实。
#[test]
fn close_observer_receives_reason_but_release_is_silent() {
    // 创建字符串测试队列。
    let queue = ToastQueue::new();
    // 保存实际关闭原因。
    let observed = Arc::new(Mutex::new(Vec::new()));
    // 克隆记录存储供观察器使用。
    let callback_observed = Arc::clone(&observed);
    // 写入带关闭观察器的声明条目。
    queue.push_external_with_close(
        11,
        "声明".to_string(),
        0,
        Arc::new(move |reason| {
            // 记录真实关闭原因。
            callback_observed
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(reason);
        }),
    );
    // 手动关闭必须发布 Manual。
    assert_eq!(
        queue.close_keys(&[ToastKey::External(11)], FeedbackCloseReason::Manual),
        1
    );
    // 重复关闭既不移除也不重复发布。
    assert_eq!(
        queue.close_keys(&[ToastKey::External(11)], FeedbackCloseReason::Manual),
        0
    );
    // 只收到一次 Manual 原因。
    assert_eq!(
        observed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_slice(),
        &[FeedbackCloseReason::Manual]
    );
    // 写入新的声明条目用于释放测试。
    queue.push_external_with_close(
        12,
        "释放".to_string(),
        0,
        Arc::new(|_| panic!("声明释放不得产生关闭事实")),
    );
    // 卸载释放只移除条目。
    assert!(queue.release_external(12));
}
