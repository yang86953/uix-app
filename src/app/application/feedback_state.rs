//! Application System 持有的逐窗反馈 owner 与声明租约表。

// 引入逐窗映射表。
#[cfg(feature = "feedback")]
use std::collections::HashMap;
// 引入稳定 ID、共享所有权与短临界区锁。
#[cfg(feature = "feedback")]
use std::sync::{Arc, Mutex, atomic::AtomicU64, atomic::Ordering};
// 引入原生错误通知创建时刻。
#[cfg(feature = "feedback")]
use std::time::Instant;

// 引入窗口稳定身份。
#[cfg(feature = "feedback")]
use crate::core::WindowId;
// 引入类型化错误结果。
#[cfg(feature = "feedback")]
use crate::core::{Errc, Error, Result};
// 引入平台中立的错误通知值。
#[cfg(feature = "feedback")]
use crate::platform::services::ToastEntry;
// 引入声明租约、关闭事实与配置。
#[cfg(feature = "feedback")]
use crate::ui::widgets::feedback::declaration::{
    FeedbackCloseReason, FeedbackClosed, FeedbackDeclarationBinding, FeedbackDeclarationSpec,
    FeedbackKind, FeedbackLease,
};
// 引入 Message 队列句柄与条目。
#[cfg(feature = "feedback")]
use crate::ui::widgets::feedback::message::{MessageHandle, MessageItem};
// 引入 Notification 队列句柄与条目。
#[cfg(feature = "feedback")]
use crate::ui::widgets::feedback::notification::{NotificationHandle, NotificationItem};

// 保存单个窗口的两类反馈队列句柄，避免在 Application System 外复制 owner。
#[cfg(feature = "feedback")]
#[derive(Clone)]
pub(super) struct AppWindowFeedback {
    // Message Host 与声明条目共享这一窗口私有队列。
    pub(super) message: MessageHandle,
    // Notification Host 与原生错误桥共享这一窗口私有队列。
    pub(super) notification: NotificationHandle,
    // 保存当前窗口的 keyed 声明租约与 tombstone。
    declarations: HashMap<(FeedbackKind, String), FeedbackDeclarationRecord>,
}

// 声明条目的 Application owner 记录。
#[cfg(feature = "feedback")]
#[derive(Clone)]
struct FeedbackDeclarationRecord {
    // 高位命名空间中的稳定队列 ID。
    id: u64,
    // 保存最新配置和 @close 回调。
    spec: FeedbackDeclarationSpec,
    // 区分可见条目与同租约关闭 tombstone。
    phase: FeedbackDeclarationPhase,
}

// 声明租约的最小 owner 状态。
#[cfg(feature = "feedback")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum FeedbackDeclarationPhase {
    // 条目仍在队列中或执行离场动画。
    Active,
    // 条目已关闭，同租约 reconcile 不得重加。
    Dismissed,
}

// Application System 持有的唯一逐窗反馈状态。
#[derive(Clone)]
pub(crate) struct AppFeedbackState {
    // 反馈能力启用时保存逐窗 Message 与 Notification 句柄。
    #[cfg(feature = "feedback")]
    windows: Arc<Mutex<HashMap<WindowId, AppWindowFeedback>>>,
    // 反馈能力启用时生成稳定的原生错误通知 ID。
    #[cfg(feature = "feedback")]
    next_id: Arc<AtomicU64>,
    // 反馈能力启用时生成与其他队列来源不冲突的声明 ID。
    #[cfg(feature = "feedback")]
    next_declaration_id: Arc<AtomicU64>,
    // 反馈能力启用时限制同时可见通知数。
    #[cfg(feature = "feedback")]
    max_visible: usize,
}

impl AppFeedbackState {
    pub(crate) fn new() -> Self {
        // 反馈能力启用时初始化完整逐窗反馈状态。
        #[cfg(feature = "feedback")]
        {
            Self {
                windows: Arc::new(Mutex::new(HashMap::new())),
                next_id: Arc::new(AtomicU64::new(0)),
                // 使用高位命名空间隔离命令式外部通知 ID。
                next_declaration_id: Arc::new(AtomicU64::new(1_u64 << 63)),
                max_visible: 5,
            }
        }
        // 关闭反馈能力时保留零尺寸 DI 哨兵，通用应用流程无需分叉。
        #[cfg(not(feature = "feedback"))]
        {
            Self {}
        }
    }

    // 反馈 capability 启用时按需创建并返回同一窗口的完整反馈句柄组。
    #[cfg(feature = "feedback")]
    pub(super) fn handles(&self, window_id: WindowId) -> AppWindowFeedback {
        // 锁中只进行轻量查找或首次队列创建，不执行用户回调。
        self.windows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entry(window_id)
            .or_insert_with(|| AppWindowFeedback {
                // 为目标窗口创建唯一 Message 队列。
                message: MessageHandle::new(),
                // 为目标窗口创建唯一 Notification 队列。
                notification: NotificationHandle::new(),
                // 新窗口尚未挂载声明租约。
                declarations: HashMap::new(),
            })
            .clone()
    }

    // 把 Rust 侧消息加入目标窗口队列。
    #[cfg(feature = "feedback")]
    pub(super) fn push_message(&self, window_id: WindowId, item: MessageItem) -> u64 {
        // 队列生成的 ID 在对应窗口和反馈类型内稳定。
        self.handles(window_id).message.add(item)
    }

    // 把 Rust 侧通知加入目标窗口队列。
    #[cfg(feature = "feedback")]
    pub(super) fn push_notification(&self, window_id: WindowId, item: NotificationItem) -> u64 {
        // 队列生成的 ID 在对应窗口和反馈类型内稳定。
        self.handles(window_id).notification.add(item)
    }

    // 为准备中的声明节点构造目标窗口窄端口。
    #[cfg(feature = "feedback")]
    pub(super) fn declaration_binding(
        &self,
        window_id: WindowId,
        kind: FeedbackKind,
    ) -> FeedbackDeclarationBinding {
        // 克隆 Application owner 供声明节点挂载时获取租约。
        let state = self.clone();
        // 端口只暴露 acquire，不暴露窗口表或队列。
        FeedbackDeclarationBinding::new(move |spec| {
            // 所有身份、重复来源与生命周期检查仍由 owner 执行。
            state.acquire_declaration(window_id, kind, spec)
        })
    }

    // 获取新的 keyed 声明租约并首次入队。
    #[cfg(feature = "feedback")]
    fn acquire_declaration(
        &self,
        window_id: WindowId,
        kind: FeedbackKind,
        spec: FeedbackDeclarationSpec,
    ) -> Result<FeedbackLease> {
        // 防止错误端口接收另一类声明配置。
        if declaration_kind(&spec) != kind {
            // 返回类型化配置错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "feedback declaration kind does not match its window binding",
            ));
        }
        // 稳定 key 必须非空且不能只含空白。
        if spec.key().trim().is_empty() {
            // 运行时 Rust 构造也遵守 UIX 的必需身份契约。
            return Err(Error::new(
                Errc::InvalidArgument,
                "feedback declaration key must not be empty",
            ));
        }
        // 先确保目标窗口句柄组已经建立。
        self.handles(window_id);
        // 保存拥有型 key 供 owner 表和租约闭包共享。
        let key = spec.key().to_string();
        // 从隔离高位命名空间分配稳定队列 ID。
        let id = self.next_declaration_id.fetch_add(1, Ordering::Relaxed);
        {
            // 锁中只验证并登记 owner 记录。
            let mut windows = self
                .windows
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            // handles 已确保目标窗口存在。
            let Some(window) = windows.get_mut(&window_id) else {
                // 若所有权表被并发破坏，则以类型化内部状态错误终止登记。
                return Err(Error::new(
                    // 该分支表示运行时状态机不一致。
                    Errc::InvalidState,
                    // 保留可定位的窗口所有权上下文。
                    "feedback window missing after handles initialization",
                ));
            };
            // 同窗口同类型同 key 只允许一个声明来源。
            if window.declarations.contains_key(&(kind, key.clone())) {
                // 拒绝重复来源，防止两个节点争用同一租约。
                return Err(Error::new(
                    Errc::AlreadyExists,
                    format!("duplicate feedback declaration key: {key}"),
                ));
            }
            // 登记活动租约。
            window.declarations.insert(
                (kind, key.clone()),
                FeedbackDeclarationRecord {
                    // 保存稳定队列 ID。
                    id,
                    // 保存初始声明配置。
                    spec: spec.clone(),
                    // 首次获取处于活动状态。
                    phase: FeedbackDeclarationPhase::Active,
                },
            );
        }
        // 锁外安装队列条目与关闭观察器。
        self.install_declaration(window_id, kind, &key, id, &spec);
        // 更新闭包持有独立 owner 克隆与稳定身份。
        let update_state = self.clone();
        // 更新闭包持有自己的 key 副本。
        let update_key = key.clone();
        // 释放闭包同样只持有窄身份。
        let release_state = self.clone();
        // 构造不暴露 owner 内部表的租约。
        Ok(FeedbackLease::new(
            // 同 key reconcile 更新现有记录。
            move |next| update_state.update_declaration(window_id, kind, &update_key, id, next),
            // 真正卸载释放记录与 tombstone。
            move || release_state.release_declaration(window_id, kind, &key, id),
        ))
    }

    // 把首次声明配置安装到对应 Host 队列。
    #[cfg(feature = "feedback")]
    fn install_declaration(
        &self,
        window_id: WindowId,
        kind: FeedbackKind,
        key: &str,
        id: u64,
        spec: &FeedbackDeclarationSpec,
    ) {
        // 取得目标窗口窄句柄组。
        let handles = self.handles(window_id);
        // 关闭观察器只回传稳定身份和原因。
        let close_state = self.clone();
        // 为异步关闭观察器保存拥有型 key。
        let close_key = key.to_string();
        // 按声明类型写入对应队列。
        match spec {
            // Message 使用顶部消息 Host。
            FeedbackDeclarationSpec::Message(spec) => handles.message.push_declaration(
                id,
                MessageItem {
                    // 保留状态等级。
                    type_: spec.type_,
                    // 复制显示内容。
                    content: spec.content.clone(),
                    // 保存毫秒展示时长。
                    duration_ms: spec.duration_ms,
                    // 保存关闭能力。
                    closable: spec.closable,
                },
                // Host 确认关闭后建立 tombstone。
                move |reason| {
                    close_state.close_declaration(window_id, kind, &close_key, id, reason);
                },
            ),
            // Notification 使用右上角通知 Host。
            FeedbackDeclarationSpec::Notification(spec) => handles.notification.push_declaration(
                id,
                NotificationItem {
                    // 保留状态等级。
                    type_: spec.type_,
                    // 复制通知标题。
                    title: spec.title.clone(),
                    // UIX content 映射运行时 description。
                    description: spec.content.clone(),
                    // 保存毫秒展示时长。
                    duration_ms: spec.duration_ms,
                    // 保存关闭能力。
                    closable: spec.closable,
                },
                // Host 确认关闭后建立 tombstone。
                move |reason| {
                    close_state.close_declaration(window_id, kind, &close_key, id, reason);
                },
            ),
        }
    }

    // 同 key reconcile 更新活动条目或 tombstone 配置。
    #[cfg(feature = "feedback")]
    fn update_declaration(
        &self,
        window_id: WindowId,
        kind: FeedbackKind,
        key: &str,
        id: u64,
        spec: FeedbackDeclarationSpec,
    ) -> Result<()> {
        // 更新不能改变租约的类型或稳定 key。
        if declaration_kind(&spec) != kind || spec.key() != key {
            // key 变化必须走 release + acquire。
            return Err(Error::new(
                Errc::InvalidArgument,
                "feedback declaration update changed its lease identity",
            ));
        }
        // 锁中更新 owner 记录并读取当前 phase。
        let phase = {
            // 获取逐窗反馈表。
            let mut windows = self
                .windows
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            // 窗口 teardown 后租约失效。
            let window = windows.get_mut(&window_id).ok_or_else(|| {
                // 返回稳定生命周期错误。
                Error::new(Errc::InvalidState, "feedback window is no longer available")
            })?;
            // 读取完全匹配的租约记录。
            let record = window
                .declarations
                .get_mut(&(kind, key.to_string()))
                .filter(|record| record.id == id)
                .ok_or_else(|| {
                    // 拒绝过期或被替换的租约。
                    Error::new(
                        Errc::InvalidState,
                        "feedback declaration lease is no longer active",
                    )
                })?;
            // 更新 tombstone 中的回调和配置，但不重新入队。
            record.spec = spec.clone();
            // 返回当前 phase 供锁外决定队列更新。
            record.phase
        };
        // 已关闭 tombstone 在同租约普通重建中保持不可见。
        if phase == FeedbackDeclarationPhase::Dismissed {
            // 配置已保存，等待真正卸载释放身份。
            return Ok(());
        }
        // 活动条目在锁外更新队列，保持原稳定 ID。
        let handles = self.handles(window_id);
        // 按类型更新对应 Host 队列。
        match spec {
            // 更新 Message 外观与时长。
            FeedbackDeclarationSpec::Message(spec) => handles.message.update_declaration(
                id,
                MessageItem {
                    // 保留状态等级。
                    type_: spec.type_,
                    // 更新内容。
                    content: spec.content,
                    // 只有该字段变化时 motion 重启计时。
                    duration_ms: spec.duration_ms,
                    // 更新关闭能力。
                    closable: spec.closable,
                },
            ),
            // 更新 Notification 外观与时长。
            FeedbackDeclarationSpec::Notification(spec) => handles.notification.update_declaration(
                id,
                NotificationItem {
                    // 保留状态等级。
                    type_: spec.type_,
                    // 更新标题。
                    title: spec.title,
                    // 更新正文。
                    description: spec.content,
                    // 只有该字段变化时 motion 重启计时。
                    duration_ms: spec.duration_ms,
                    // 更新关闭能力。
                    closable: spec.closable,
                },
            ),
        }
        // 更新成功。
        Ok(())
    }

    // Host 确认关闭后建立同租约 tombstone 并发布事实。
    #[cfg(feature = "feedback")]
    fn close_declaration(
        &self,
        window_id: WindowId,
        kind: FeedbackKind,
        key: &str,
        id: u64,
        reason: FeedbackCloseReason,
    ) {
        // 锁中只更新 phase 并复制用户回调。
        let callback = {
            // 获取逐窗反馈表。
            let mut windows = self
                .windows
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            // teardown 后到达的关闭事实安全丢弃。
            let Some(window) = windows.get_mut(&window_id) else {
                // 窗口已失效，无用户回调可达。
                return;
            };
            // 只接受当前租约的关闭事实。
            let Some(record) = window
                .declarations
                .get_mut(&(kind, key.to_string()))
                .filter(|record| record.id == id)
            else {
                // 过期 Host 事实不得关闭新租约。
                return;
            };
            // 重复关闭保持幂等且只发布一次事实。
            if record.phase == FeedbackDeclarationPhase::Dismissed {
                // 已经存在 tombstone。
                return;
            }
            // 先建立 tombstone，再执行用户回调。
            record.phase = FeedbackDeclarationPhase::Dismissed;
            // 从最新声明配置读取回调。
            declaration_close_callback(&record.spec)
        };
        // 锁外发布类型化关闭事实。
        if let Some(callback) = callback {
            // 回调收到稳定 key 与真实原因。
            callback(FeedbackClosed {
                // 复制稳定身份。
                key: key.to_string(),
                // 保存 Host 给出的原因。
                reason,
            });
        }
    }

    // 真正卸载释放 owner 记录、tombstone 与队列条目。
    #[cfg(feature = "feedback")]
    fn release_declaration(&self, window_id: WindowId, kind: FeedbackKind, key: &str, id: u64) {
        // 锁中仅在身份完全匹配时删除记录。
        let handles = {
            // 获取逐窗反馈表。
            let mut windows = self
                .windows
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            // teardown 已整体释放，无需重复处理。
            let Some(window) = windows.get_mut(&window_id) else {
                // 窗口已不存在。
                return;
            };
            // 防止过期租约释放同 key 的新记录。
            if !window
                .declarations
                .get(&(kind, key.to_string()))
                .is_some_and(|record| record.id == id)
            {
                // 身份不匹配时保持当前记录。
                return;
            }
            // 移除活动记录或 tombstone。
            window.declarations.remove(&(kind, key.to_string()));
            // 克隆两个窄句柄供锁外释放队列。
            AppWindowFeedback {
                // 共享 Message 队列。
                message: window.message.clone(),
                // 共享 Notification 队列。
                notification: window.notification.clone(),
                // 锁外不需要声明表副本。
                declarations: HashMap::new(),
            }
        };
        // 卸载不产生 @close 事实。
        match kind {
            // 从 Message 队列释放外部 ID。
            FeedbackKind::Message => {
                // 忽略已经因关闭而离开队列的情况。
                handles.message.release_declaration(id);
            }
            // 从 Notification 队列释放外部 ID。
            FeedbackKind::Notification => {
                // 忽略已经因关闭而离开队列的情况。
                handles.notification.release_declaration(id);
            }
        }
    }

    // 反馈 capability 启用时才把应用错误加入通知队列。
    #[cfg(feature = "feedback")]
    pub(crate) fn notify_error(&self, window_id: WindowId, error: &Error) -> Option<u64> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let toast = ToastEntry::from_error(id, error, Instant::now())?;
        let item = NotificationItem::from_toast_entry(&toast);
        // 原生错误桥只写入目标窗口的 Notification 队列。
        let handle = self.handles(window_id).notification;
        handle.push_external(id, item);
        handle.retain_latest(self.max_visible);
        Some(id)
    }

    // 反馈 capability 启用时才清理逐窗反馈状态。
    #[cfg(feature = "feedback")]
    pub(crate) fn remove_window(&self, window_id: WindowId) {
        self.windows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&window_id);
    }
}

// 返回声明配置所属的反馈类型。
#[cfg(feature = "feedback")]
fn declaration_kind(spec: &FeedbackDeclarationSpec) -> FeedbackKind {
    // 两个枚举保持一一对应。
    match spec {
        // Message 配置只进入 Message 队列。
        FeedbackDeclarationSpec::Message(_) => FeedbackKind::Message,
        // Notification 配置只进入 Notification 队列。
        FeedbackDeclarationSpec::Notification(_) => FeedbackKind::Notification,
    }
}

// 从最新声明配置复制类型化关闭回调。
#[cfg(feature = "feedback")]
fn declaration_close_callback(
    spec: &FeedbackDeclarationSpec,
) -> Option<Arc<dyn Fn(FeedbackClosed) + Send + Sync>> {
    // 两类配置共享相同关闭事实类型。
    match spec {
        // 复制 Message 关闭回调。
        FeedbackDeclarationSpec::Message(spec) => spec.on_close.clone(),
        // 复制 Notification 关闭回调。
        FeedbackDeclarationSpec::Notification(spec) => spec.on_close.clone(),
    }
}
