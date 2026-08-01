//! 进程级反馈门面 — `ui::message()` / `ui::notify()`（E-04）。
//!
//! 任意代码位置（事件回调、定时器、CLI 上下文）可取用当前注册的反馈管理器，
//! 无需持有组件实例。注册语义：`Message::new()` / `Notification::new()` 构造时
//! 自动注册为当前反馈目标（逐窗隔离：每窗口一个组件实例，最近挂载生效）；
//! App 组合根可显式 [`register_feedback`] 覆盖。
//!
//! 未注册时门面为 no-op（返回 0），`is_available()` 可查询；不 panic、不静默
//! 吞掉应用错误。

use std::sync::{OnceLock, RwLock};

use crate::ui::widgets::feedback::message::{MessageHandle, MessageItem};
use crate::ui::widgets::feedback::notification::NotificationHandle;

/// 当前进程的反馈注册表。
#[derive(Default)]
struct FeedbackRegistry {
    message: Option<MessageHandle>,
    notification: Option<NotificationHandle>,
}

static FEEDBACK: OnceLock<RwLock<FeedbackRegistry>> = OnceLock::new();

fn registry() -> &'static RwLock<FeedbackRegistry> {
    FEEDBACK.get_or_init(|| RwLock::new(FeedbackRegistry::default()))
}

/// 注册当前窗口的反馈管理器（消息 + 通知）；覆盖既有注册。
pub fn register_feedback(message: MessageHandle, notification: NotificationHandle) {
    let mut current = registry().write().unwrap();
    current.message = Some(message);
    current.notification = Some(notification);
}

/// 注册当前窗口的消息管理器（`Message::new()` 构造时自动调用）。
pub fn register_message(message: MessageHandle) {
    registry().write().unwrap().message = Some(message);
}

/// 注册当前窗口的通知管理器（`Notification::new()` 构造时自动调用）。
pub fn register_notification(notification: NotificationHandle) {
    registry().write().unwrap().notification = Some(notification);
}

/// 清除当前反馈注册（窗口销毁时由组合根调用）。
pub fn unregister_feedback() {
    let mut current = registry().write().unwrap();
    current.message = None;
    current.notification = None;
}

/// 全局消息门面（E-04）：`ui::message().success("已保存")`。
#[derive(Clone)]
pub struct MessageFacade {
    handle: Option<MessageHandle>,
}

impl MessageFacade {
    /// 当前是否有已注册的消息管理器。
    pub fn is_available(&self) -> bool {
        self.handle.is_some()
    }

    /// 添加成功提示并返回稳定 ID；未注册时返回 0。
    pub fn success(&self, content: impl Into<String>) -> u64 {
        self.handle
            .as_ref()
            .map_or(0, |handle| handle.success(content))
    }

    /// 添加信息提示并返回稳定 ID；未注册时返回 0。
    pub fn info(&self, content: impl Into<String>) -> u64 {
        self.handle
            .as_ref()
            .map_or(0, |handle| handle.info(content))
    }

    /// 添加警告提示并返回稳定 ID；未注册时返回 0。
    pub fn warning(&self, content: impl Into<String>) -> u64 {
        self.handle
            .as_ref()
            .map_or(0, |handle| handle.warning(content))
    }

    /// 添加错误提示并返回稳定 ID；未注册时返回 0。
    pub fn error(&self, content: impl Into<String>) -> u64 {
        self.handle
            .as_ref()
            .map_or(0, |handle| handle.error(content))
    }

    /// 添加自定义提示项并返回稳定 ID；未注册时返回 0。
    pub fn add(&self, item: MessageItem) -> u64 {
        self.handle.as_ref().map_or(0, |handle| handle.add(item))
    }
}

/// 全局通知门面（E-04）：`ui::notify().info("标题", "描述")`。
#[derive(Clone)]
pub struct NotificationFacade {
    handle: Option<NotificationHandle>,
}

impl NotificationFacade {
    /// 当前是否有已注册的通知管理器。
    pub fn is_available(&self) -> bool {
        self.handle.is_some()
    }

    /// 添加成功通知并返回稳定 ID；未注册时返回 0。
    pub fn success(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.handle
            .as_ref()
            .map_or(0, |handle| handle.success(title, description))
    }

    /// 添加信息通知并返回稳定 ID；未注册时返回 0。
    pub fn info(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.handle
            .as_ref()
            .map_or(0, |handle| handle.info(title, description))
    }

    /// 添加警告通知并返回稳定 ID；未注册时返回 0。
    pub fn warning(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.handle
            .as_ref()
            .map_or(0, |handle| handle.warning(title, description))
    }

    /// 添加错误通知并返回稳定 ID；未注册时返回 0。
    pub fn error(&self, title: impl Into<String>, description: impl Into<String>) -> u64 {
        self.handle
            .as_ref()
            .map_or(0, |handle| handle.error(title, description))
    }
}

/// 取用当前窗口的消息门面（E-04）。
pub fn message() -> MessageFacade {
    MessageFacade {
        handle: registry().read().unwrap().message.clone(),
    }
}

/// 取用当前窗口的通知门面（E-04）。
pub fn notify() -> NotificationFacade {
    NotificationFacade {
        handle: registry().read().unwrap().notification.clone(),
    }
}
