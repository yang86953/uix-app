// 引入被测 owner 与私有状态。
use super::*;
// 引入反馈状态等级。
use crate::platform::capabilities::StatusLevel;
// 引入线程安全关闭事实观察存储。
use std::sync::Mutex;

// 构造最小 Message 声明配置。
fn message_spec(
    key: &str,
    content: &str,
    closed: Arc<Mutex<Vec<FeedbackClosed>>>,
) -> FeedbackDeclarationSpec {
    // 返回拥有型 Message 配置。
    FeedbackDeclarationSpec::Message(
        crate::ui::widgets::feedback::declaration::MessageDeclarationSpec {
            // 保存稳定 key。
            key: key.to_string(),
            // 使用信息状态。
            type_: StatusLevel::Info,
            // 保存可观察内容。
            content: content.to_string(),
            // 零时长避免测试计时器。
            duration_ms: 0,
            // 允许程序化关闭。
            closable: true,
            // 记录类型化关闭事实。
            on_close: Some(Arc::new(move |fact| {
                // 测试锁只保护短 Vec 追加。
                closed
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(fact);
            })),
        },
    )
}

// 验证关闭后的同租约更新不重入队，卸载后重挂可再次显示。
#[test]
fn declaration_tombstone_survives_reconcile_until_release() {
    // 创建 Application System 唯一反馈 owner。
    let feedback = AppFeedbackState::new();
    // 保存关闭事实供断言。
    let closed = Arc::new(Mutex::new(Vec::new()));
    // 获取根窗口 Message 声明租约。
    let lease = feedback
        .acquire_declaration(
            WindowId::ROOT,
            FeedbackKind::Message,
            message_spec("saved", "初始", Arc::clone(&closed)),
        )
        .expect("首次稳定 key 应获取租约");
    // 首次挂载只产生一个队列条目。
    assert_eq!(feedback.handles(WindowId::ROOT).message.len(), 1);
    // 从 owner 表读取声明的高位稳定 ID。
    let id = feedback
        .windows
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&WindowId::ROOT)
        .and_then(|window| {
            window
                .declarations
                .get(&(FeedbackKind::Message, "saved".to_string()))
        })
        .map(|record| record.id)
        .expect("owner 必须保存声明 ID");
    // 程序化关闭进入与 Host 相同的关闭事实路径。
    assert!(
        feedback
            .handles(WindowId::ROOT)
            .message
            .close_declaration(id)
    );
    // 队列条目已经离开。
    assert_eq!(feedback.handles(WindowId::ROOT).message.len(), 0);
    // 关闭事实只发布一次并保留稳定 key 与原因。
    assert_eq!(
        closed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_slice(),
        &[FeedbackClosed {
            // 关闭事实返回声明 key。
            key: "saved".to_string(),
            // 程序化入口返回 Programmatic。
            reason: FeedbackCloseReason::Programmatic,
        }]
    );
    // 普通 reconcile 更新 tombstone 配置。
    lease
        .update(message_spec("saved", "更新", Arc::clone(&closed)))
        .expect("同租约更新应保持有效");
    // tombstone 不得因普通重建重复入队。
    assert_eq!(feedback.handles(WindowId::ROOT).message.len(), 0);
    // 同一租约未释放前，重复来源必须被拒绝。
    let duplicate = feedback
        .acquire_declaration(
            WindowId::ROOT,
            FeedbackKind::Message,
            message_spec("saved", "重复", Arc::clone(&closed)),
        )
        // 只提取失败分支，FeedbackLease 不需要 Debug。
        .err()
        .expect("同窗口同类型同 key 必须唯一");
    // 重复来源使用稳定 AlreadyExists 错误码。
    assert_eq!(duplicate.code(), Errc::AlreadyExists);
    // 真正卸载释放 tombstone。
    lease.release();
    // 卸载后重挂同 key 可以再次显示。
    let remounted = feedback
        .acquire_declaration(
            WindowId::ROOT,
            FeedbackKind::Message,
            message_spec("saved", "重挂", closed),
        )
        .expect("释放后重挂应获取新租约");
    // 新租约重新进入队列。
    assert_eq!(feedback.handles(WindowId::ROOT).message.len(), 1);
    // 测试结束显式释放新租约。
    remounted.release();
}
