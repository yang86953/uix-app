// Agent Bridge 目录核心逻辑测试。
// 覆盖目录状态机（启用/注册/列举/关闭）与等待条件判定，全部为内存内同步操作，
// 不启动线程与 IO；wait 仅验证错误路径与短超时路径。

// 引入被测的目录、等待条件/结果与端口发布 trait。
use super::{
    AgentBridgeDirectory, AgentSemanticsPort, AgentWaitCondition, AgentWaitError, AgentWaitOutcome,
    AgentWindowInfo, wait_outcome,
};
// 引入核心标识与语义快照类型。
use crate::app::window_semantics::WindowSemanticSnapshot;
use crate::core::WindowId;

/// 构造一个已启用且注册了单窗口的目录。
fn enabled_directory_with_window(window_id: WindowId) -> (AgentBridgeDirectory, u64) {
    let directory = AgentBridgeDirectory::default();
    // 启用目录后才允许注册窗口。
    assert!(directory.enable());
    let registration = directory
        .register_window(window_id, "主窗口".to_owned(), true, true)
        .expect("启用后注册窗口必须成功");
    (directory, registration.generation)
}

// 目录必须在启用后接受注册，且重复启用被拒绝。
#[test]
fn directory_enable_is_idempotent_once() {
    let directory = AgentBridgeDirectory::default();
    // 首次启用必须成功。
    assert!(directory.enable());
    // 重复启用必须被拒绝。
    assert!(!directory.enable());
    // 启用状态必须可查询。
    assert!(directory.is_enabled());
}

// 未启用的目录必须拒绝注册窗口。
#[test]
fn register_window_requires_enabled_directory() {
    let directory = AgentBridgeDirectory::default();
    // 未启用时注册必须返回 None。
    assert!(
        directory
            .register_window(WindowId::new(1), "x".to_owned(), true, true)
            .is_none()
    );
}

// 同窗口重复注册必须递增代次。
#[test]
fn re_registration_increments_generation() {
    let directory = AgentBridgeDirectory::default();
    // 启用目录。
    assert!(directory.enable());
    // 首次注册代次必须为 1。
    let first = directory
        .register_window(WindowId::new(1), "a".to_owned(), true, true)
        .expect("首次注册必须成功");
    assert_eq!(first.generation, 1);
    // 再次注册同窗口代次必须递增。
    let second = directory
        .register_window(WindowId::new(1), "b".to_owned(), true, true)
        .expect("重复注册必须成功");
    assert_eq!(second.generation, 2);
    // 旧注册的窗口信息必须被新信息替换。
    let windows = directory.list_windows().expect("列举必须成功");
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].generation, 2);
    assert_eq!(windows[0].title, "b");
}

// list_windows 必须只返回存活窗口并携带完整元数据。
#[test]
fn list_windows_reports_live_windows_with_metadata() {
    // 注册两个窗口。
    let directory = AgentBridgeDirectory::default();
    assert!(directory.enable());
    directory.register_window(WindowId::new(1), "甲".to_owned(), true, false);
    directory.register_window(WindowId::new(2), "乙".to_owned(), false, true);
    // 列举必须按注册顺序返回两条记录。
    let windows = directory.list_windows().expect("列举必须成功");
    assert_eq!(windows.len(), 2);
    // 窗口元数据必须完整。
    assert_eq!(windows[0].window_id, WindowId::new(1));
    assert_eq!(windows[0].title, "甲");
    assert!(windows[0].visible);
    assert!(!windows[0].presentable);
    assert!(!windows[0].closed);
    assert_eq!(windows[0].generation, 1);
    // 第二个窗口的可见性相反。
    assert!(!windows[1].visible);
    assert!(windows[1].presentable);
}

// close_window 必须标记关闭，重复关闭幂等，已关闭窗口不再存活。
#[test]
fn close_window_marks_closed_and_is_idempotent() {
    let window_id = WindowId::new(7);
    let (directory, _) = enabled_directory_with_window(window_id);
    // 关闭窗口必须成功。
    assert!(directory.close_window(window_id));
    // 重复关闭必须幂等返回 false。
    assert!(!directory.close_window(window_id));
    // 已关闭窗口必须从列表消失。
    let windows = directory.list_windows().expect("列举必须成功");
    assert!(windows.is_empty(), "已关闭窗口必须从列表消失");
    // 关闭不存在的窗口必须返回 false。
    assert!(!directory.close_window(WindowId::new(999)));
    // 已关闭窗口不再视为存活。
    assert!(!directory.contains_live_window(window_id));
}

// close_all 必须关闭全部窗口并使目录进入关闭态。
#[test]
fn close_all_closes_windows_and_flags_app_closed() {
    let directory = AgentBridgeDirectory::default();
    // 启用并注册两个窗口。
    assert!(directory.enable());
    directory.register_window(WindowId::new(1), "a".to_owned(), true, true);
    directory.register_window(WindowId::new(2), "b".to_owned(), true, true);
    // 全部关闭。
    directory.close_all();
    // 关闭后列举必须报应用已关闭。
    assert!(matches!(
        directory.list_windows(),
        Err(crate::app::queues::agent_command_queue::AgentSubmitError::AppClosed)
    ));
    // 关闭后不允许再启用。
    assert!(!directory.enable());
    // 关闭后注册被拒绝。
    assert!(
        directory
            .register_window(WindowId::new(3), "c".to_owned(), true, true)
            .is_none()
    );
}

// 发布语义快照必须同步修订号并保持窗口存活标记。
#[test]
fn publish_semantics_updates_revision_metadata() {
    let window_id = WindowId::new(5);
    let (directory, _) = enabled_directory_with_window(window_id);
    // 重新注册以取得与发布快照一致的代次。
    let registration = directory
        .register_window(window_id, "x".to_owned(), true, true)
        .expect("重新注册必须成功");
    // 构造与注册代次一致的语义快照。
    let snapshot = WindowSemanticSnapshot {
        window_id,
        generation: registration.generation,
        revision: 9,
        presented_revision: 8,
        closed: false,
        nodes: Vec::new(),
    };
    // 通过注册端口发布语义快照。
    registration.publish_semantics(&snapshot);
    // 目录中的窗口信息必须同步修订号。
    let windows = directory.list_windows().expect("列举必须成功");
    assert_eq!(windows[0].revision, 9);
    assert_eq!(windows[0].presented_revision, 8);
    assert!(!windows[0].closed);
}

// 发布关闭快照必须把窗口标记为不可呈现并关闭。
#[test]
fn publish_closed_snapshot_marks_window_closed() {
    let window_id = WindowId::new(5);
    let (directory, _) = enabled_directory_with_window(window_id);
    // 重新注册以取得与发布快照一致的代次。
    let registration = directory
        .register_window(window_id, "x".to_owned(), true, true)
        .expect("重新注册必须成功");
    // 构造关闭状态的语义快照。
    let snapshot = WindowSemanticSnapshot {
        window_id,
        generation: registration.generation,
        revision: 2,
        presented_revision: 2,
        closed: true,
        nodes: Vec::new(),
    };
    // 通过注册端口发布关闭快照。
    registration.publish_semantics(&snapshot);
    // 窗口必须从存活列表中消失。
    assert!(!directory.contains_live_window(window_id));
}

// 过期代次的发布必须被忽略。
#[test]
fn publish_semantics_ignores_stale_generation() {
    let window_id = WindowId::new(5);
    let (directory, _) = enabled_directory_with_window(window_id);
    // 用错误的代次发布快照。
    let snapshot = WindowSemanticSnapshot {
        window_id,
        generation: 999,
        revision: 9,
        presented_revision: 9,
        closed: false,
        nodes: Vec::new(),
    };
    // 通过注册端口发布过期快照。
    let registration = directory
        .register_window(window_id, "x".to_owned(), true, true)
        .expect("重新注册必须成功");
    registration.publish_semantics(&snapshot);
    // 过期发布不得改变窗口修订号。
    let windows = directory.list_windows().expect("列举必须成功");
    assert_eq!(windows[0].revision, 0);
}

// ── wait 错误路径（不阻塞） ───────────────────────────────────────────────

// 等待未注册窗口必须立即返回 WindowNotFound。
#[test]
fn wait_unknown_window_returns_not_found() {
    let (directory, generation) = enabled_directory_with_window(WindowId::new(1));
    // 等待不存在的窗口。
    let outcome = directory.wait(
        WindowId::new(999),
        generation,
        AgentWaitCondition::RevisionAfter(0),
        std::time::Duration::from_millis(1),
    );
    // 必须报窗口不存在。
    assert!(matches!(outcome, Err(AgentWaitError::WindowNotFound)));
}

// 超长超时必须返回 InvalidTimeout。
#[test]
fn wait_oversized_timeout_is_rejected() {
    let window_id = WindowId::new(1);
    let (directory, generation) = enabled_directory_with_window(window_id);
    // 超过协议上限的超时。
    let outcome = directory.wait(
        window_id,
        generation,
        AgentWaitCondition::RevisionAfter(0),
        std::time::Duration::from_secs(31),
    );
    // 必须报超时超限。
    assert!(matches!(
        outcome,
        Err(AgentWaitError::InvalidTimeout { .. })
    ));
}

// 代次不匹配必须返回 StaleWindow。
#[test]
fn wait_stale_generation_returns_stale_window() {
    let window_id = WindowId::new(1);
    let (directory, _) = enabled_directory_with_window(window_id);
    // 用错误的代次等待。
    let outcome = directory.wait(
        window_id,
        999,
        AgentWaitCondition::RevisionAfter(0),
        std::time::Duration::from_millis(1),
    );
    // 必须报窗口代次过期。
    assert!(matches!(
        outcome,
        Err(AgentWaitError::StaleWindow { expected: 999, .. })
    ));
}

// 目录关闭后等待必须返回 AppClosed。
#[test]
fn wait_after_close_all_returns_app_closed() {
    let window_id = WindowId::new(1);
    let (directory, generation) = enabled_directory_with_window(window_id);
    // 关闭全部窗口。
    directory.close_all();
    // 关闭后等待必须报应用已关闭。
    let outcome = directory.wait(
        window_id,
        generation,
        AgentWaitCondition::RevisionAfter(0),
        std::time::Duration::from_millis(1),
    );
    // 必须报应用关闭。
    assert!(matches!(outcome, Err(AgentWaitError::AppClosed)));
}

// 条件未满足时等待必须按超时返回 Timeout。
#[test]
fn wait_unmet_condition_times_out() {
    let window_id = WindowId::new(1);
    let (directory, generation) = enabled_directory_with_window(window_id);
    // 等待一个远高于当前修订号的条件。
    let outcome = directory.wait(
        window_id,
        generation,
        AgentWaitCondition::RevisionAfter(999_999),
        std::time::Duration::from_millis(1),
    );
    // 无发布者时短超时必须返回 Timeout。
    assert!(matches!(outcome, Err(AgentWaitError::Timeout)));
}

// ── wait_outcome 条件判定（纯函数） ──────────────────────────────────────

/// 构造一个指定修订号的存活窗口信息。
fn window_with_revisions(revision: u64, presented: u64, closed: bool) -> AgentWindowInfo {
    AgentWindowInfo {
        window_id: WindowId::new(1),
        generation: 1,
        title: "w".to_owned(),
        visible: true,
        presentable: !closed,
        revision,
        presented_revision: presented,
        closed,
    }
}

// RevisionAfter 必须在修订号越过阈值时返回 Changed。
#[test]
fn wait_outcome_revision_after_threshold() {
    // 修订号必须严格大于阈值。
    assert_eq!(
        wait_outcome(
            &window_with_revisions(5, 0, false),
            AgentWaitCondition::RevisionAfter(4)
        ),
        Some(AgentWaitOutcome::Changed(window_with_revisions(
            5, 0, false
        )))
    );
    // 修订号等于阈值不满足条件。
    assert_eq!(
        wait_outcome(
            &window_with_revisions(5, 0, false),
            AgentWaitCondition::RevisionAfter(5)
        ),
        None
    );
    // 修订号低于阈值不满足条件。
    assert_eq!(
        wait_outcome(
            &window_with_revisions(3, 0, false),
            AgentWaitCondition::RevisionAfter(4)
        ),
        None
    );
}

// PresentedAtLeast 必须在呈现修订号达到阈值时返回 Presented。
#[test]
fn wait_outcome_presented_at_least_threshold() {
    // 呈现修订号等于阈值必须满足。
    assert_eq!(
        wait_outcome(
            &window_with_revisions(0, 4, false),
            AgentWaitCondition::PresentedAtLeast(4)
        ),
        Some(AgentWaitOutcome::Presented(window_with_revisions(
            0, 4, false
        )))
    );
    // 呈现修订号高于阈值必须满足。
    assert_eq!(
        wait_outcome(
            &window_with_revisions(0, 6, false),
            AgentWaitCondition::PresentedAtLeast(4)
        ),
        Some(AgentWaitOutcome::Presented(window_with_revisions(
            0, 6, false
        )))
    );
    // 呈现修订号低于阈值不满足。
    assert_eq!(
        wait_outcome(
            &window_with_revisions(0, 2, false),
            AgentWaitCondition::PresentedAtLeast(4)
        ),
        None
    );
}

// 关闭窗口必须优先返回 Closed，即使修订条件已满足。
#[test]
fn wait_outcome_closed_wins_over_revision_condition() {
    // 已关闭窗口即使满足修订条件也必须返回 Closed。
    let closed = window_with_revisions(9, 9, true);
    assert_eq!(
        wait_outcome(&closed, AgentWaitCondition::RevisionAfter(0)),
        Some(AgentWaitOutcome::Closed(closed.clone()))
    );
    assert_eq!(
        wait_outcome(&closed, AgentWaitCondition::PresentedAtLeast(0)),
        Some(AgentWaitOutcome::Closed(closed))
    );
}
