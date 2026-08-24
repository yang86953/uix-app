//! 通知条目与挂载句柄。

use crate::platform::capabilities::StatusLevel;
use crate::platform::services::ToastEntry;

use super::Notification;
use crate::ui::widgets::feedback::toast_motion::ToastQueue;
// 引入声明条目关闭原因。
use crate::ui::widgets::feedback::declaration::FeedbackCloseReason;

#[derive(Debug, Clone)]
/// 通知队列中可呈现的状态、文本和关闭策略。
pub struct NotificationItem {
    /// 决定图标和强调色的通知状态等级。
    pub type_: StatusLevel,
    /// 通知的主标题。
    pub title: String,
    /// 通知标题下显示的说明正文。
    pub description: String,
    /// 展示时长；`0` 表示仅由调用方或关闭按钮移除。
    pub duration_ms: u64,
    /// 指示是否向用户提供手动关闭入口。
    pub closable: bool,
}

impl NotificationItem {
    /// 从服务 Toast 复制内容、状态、时长，并创建可手动关闭的通知条目。
    pub fn from_toast_entry(toast: &ToastEntry) -> Self {
        Self {
            type_: toast.level,
            title: toast.title.clone(),
            description: toast.message.clone(),
            duration_ms: toast.duration_ms as u64,
            closable: true,
        }
    }
}

/// 向已挂载的 [`Notification`] 队列增删通知。
#[derive(Clone)]
pub struct NotificationHandle {
    pub(super) queue: ToastQueue<NotificationItem>,
}

impl NotificationHandle {
    pub(crate) fn new() -> Self {
        Self {
            queue: ToastQueue::new(),
        }
    }

    /// 添加通知并返回稳定 ID。
    pub fn add(&self, item: NotificationItem) -> u64 {
        let duration_ms = item.duration_ms;
        self.queue.push(item, duration_ms)
    }

    // 首次写入带关闭观察器的 keyed 声明条目。
    pub(crate) fn push_declaration<F>(&self, id: u64, item: NotificationItem, on_close: F)
    where
        // Host 可能在任意 UI 生命周期边界调用观察器。
        F: Fn(FeedbackCloseReason) + Send + Sync + 'static,
    {
        // 保存时长供队列调度。
        let duration_ms = item.duration_ms;
        // 外部高位 ID 与命令式本地 ID 分离。
        self.queue
            .push_external_with_close(id, item, duration_ms, std::sync::Arc::new(on_close));
    }

    // 幂等更新 keyed 声明条目。
    pub(crate) fn update_declaration(&self, id: u64, item: NotificationItem) {
        // 保存时长供队列判断是否重启计时。
        let duration_ms = item.duration_ms;
        // 保留现有关闭观察器并替换配置。
        self.queue.update_external(id, item, duration_ms);
    }

    // 声明卸载时无关闭事实地释放条目。
    pub(crate) fn release_declaration(&self, id: u64) -> bool {
        // 释放外部稳定 key 与观察器。
        self.queue.release_external(id)
    }

    /// 使用给定标题、描述和状态创建一条可关闭的默认时长通知。
    pub fn open(
        &self,
        title: impl Into<String>,
        description: impl Into<String>,
        type_: StatusLevel,
    ) -> u64 {
        self.add(NotificationItem {
            type_,
            title: title.into(),
            description: description.into(),
            duration_ms: Notification::default_duration_ms(),
            closable: true,
        })
    }

    /// 创建成功状态通知并返回本地稳定 ID。
    pub fn success(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.open(title, description, StatusLevel::Success)
    }

    /// 创建信息状态通知并返回本地稳定 ID。
    pub fn info(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.open(title, description, StatusLevel::Info)
    }

    /// 创建警告状态通知并返回本地稳定 ID。
    pub fn warning(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.open(title, description, StatusLevel::Warning)
    }

    /// 创建错误状态通知并返回本地稳定 ID。
    pub fn error(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.open(title, description, StatusLevel::Error)
    }

    /// 请求移除由此 handle 添加的通知。
    pub fn dismiss(&self, id: u64) -> bool {
        self.queue.remove_local(id)
    }

    /// 请求移除由通知源协议提供稳定 ID 的通知。
    pub fn dismiss_external(&self, id: u64) -> bool {
        self.queue.remove_external(id)
    }

    /// 清空共享队列中的全部通知。
    pub fn clear(&self) {
        self.queue.clear();
    }

    /// 返回共享队列中通知条目的快照。
    pub fn items(&self) -> Vec<NotificationItem> {
        self.queue.values()
    }

    /// 返回共享队列中的通知数量。
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// 返回共享队列当前是否没有通知。
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// 用可见的服务 Toast 替换外部通知集合，并保留本地通知。
    pub fn replace_from_toasts<'a, I>(&self, toasts: I)
    where
        I: IntoIterator<Item = &'a ToastEntry>,
    {
        self.queue
            .replace_external(
                toasts
                    .into_iter()
                    .filter(|toast| toast.visible)
                    .map(|toast| {
                        (
                            toast.id,
                            NotificationItem::from_toast_entry(toast),
                            u64::from(toast.duration_ms),
                        )
                    }),
            );
    }

    pub(crate) fn push_external(&self, id: u64, item: NotificationItem) {
        let duration_ms = item.duration_ms;
        self.queue.push_external(id, item, duration_ms);
    }

    pub(crate) fn retain_latest(&self, maximum: usize) {
        self.queue.retain_latest(maximum);
    }
}
