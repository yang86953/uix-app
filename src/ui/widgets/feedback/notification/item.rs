//! 通知条目与挂载句柄。

use crate::native::capabilities::system::StatusLevel;
use crate::native::notification::ToastEntry;

use crate::ui::widgets::feedback::toast_motion::ToastQueue;
use super::Notification;

#[derive(Debug, Clone)]
pub struct NotificationItem {
    pub type_: StatusLevel,
    pub title: String,
    pub description: String,
    /// 展示时长；`0` 表示仅由调用方或关闭按钮移除。
    pub duration_ms: u64,
    pub closable: bool,
}

impl NotificationItem {
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
            duration_ms: Notification::DEFAULT_DURATION_MS,
            closable: true,
        })
    }

    pub fn success(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.open(title, description, StatusLevel::Success)
    }

    pub fn info(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.open(title, description, StatusLevel::Info)
    }

    pub fn warning(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.open(title, description, StatusLevel::Warning)
    }

    pub fn error(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.open(title, description, StatusLevel::Error)
    }

    /// 请求移除由此 handle 添加的通知。
    pub fn dismiss(&self, id: u64) -> bool {
        self.queue.remove_local(id)
    }

    /// 请求移除由 [`NotificationService`] 提供稳定 ID 的通知。
    pub fn dismiss_external(&self, id: u64) -> bool {
        self.queue.remove_external(id)
    }

    pub fn clear(&self) {
        self.queue.clear();
    }

    pub fn items(&self) -> Vec<NotificationItem> {
        self.queue.values()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

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
