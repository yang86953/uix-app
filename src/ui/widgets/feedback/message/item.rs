//! 消息条目与挂载句柄。

use crate::native::capabilities::system::StatusLevel;
use crate::ui::widgets::feedback::toast_motion::ToastQueue;

use super::Message;

#[derive(Debug, Clone)]
pub struct MessageItem {
    pub type_: StatusLevel,
    pub content: String,
    /// 展示时长；`0` 表示仅由调用方或关闭按钮移除。
    pub duration_ms: u64,
    pub closable: bool,
}

/// 向已挂载的 [`Message`] 队列增删提示，不暴露内部动画与定时器。
#[derive(Clone)]
pub struct MessageHandle {
    pub(super) queue: ToastQueue<MessageItem>,
}

impl MessageHandle {
    /// 添加提示并返回稳定 ID。
    pub fn add(&self, item: MessageItem) -> u64 {
        let duration_ms = item.duration_ms;
        self.queue.push(item, duration_ms)
    }

    pub fn success(&self, content: impl Into<String>) -> u64 {
        self.add(MessageItem {
            type_: StatusLevel::Success,
            content: content.into(),
            duration_ms: Message::MSG_DURATION_SUCCESS,
            closable: true,
        })
    }

    pub fn info(&self, content: impl Into<String>) -> u64 {
        self.add(MessageItem {
            type_: StatusLevel::Info,
            content: content.into(),
            duration_ms: Message::MSG_DURATION_INFO,
            closable: true,
        })
    }

    pub fn warning(&self, content: impl Into<String>) -> u64 {
        self.add(MessageItem {
            type_: StatusLevel::Warning,
            content: content.into(),
            duration_ms: Message::MSG_DURATION_WARNING,
            closable: true,
        })
    }

    pub fn error(&self, content: impl Into<String>) -> u64 {
        self.add(MessageItem {
            type_: StatusLevel::Error,
            content: content.into(),
            duration_ms: Message::MSG_DURATION_ERROR,
            closable: true,
        })
    }

    /// 请求移除指定提示；离场动画完成后才从画面消失。
    pub fn dismiss(&self, id: u64) -> bool {
        self.queue.remove_local(id)
    }

    /// 请求移除全部提示。
    pub fn clear(&self) {
        self.queue.clear();
    }

    pub fn items(&self) -> Vec<MessageItem> {
        self.queue.values()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
