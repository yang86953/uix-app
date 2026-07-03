//! Fake 通知 — 记录通知历史，支持断言。

use crate::api::traits::INotification;

#[derive(Debug, Clone)]
pub struct NotificationRecord {
    pub title: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct FakeNotificationState {
    pub history: Vec<NotificationRecord>,
}

impl Default for FakeNotificationState {
    fn default() -> Self {
        Self {
            history: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct FakeNotification {
    pub state: FakeNotificationState,
}

impl FakeNotification {
    pub fn new() -> Self {
        Self {
            state: FakeNotificationState::default(),
        }
    }

    /// 通知历史条数
    pub fn count(&self) -> usize {
        self.state.history.len()
    }

    /// 最近一条通知
    pub fn last(&self) -> Option<&NotificationRecord> {
        self.state.history.last()
    }

    /// 按标题查找通知
    pub fn find_by_title(&self, title: &str) -> Vec<&NotificationRecord> {
        self.state
            .history
            .iter()
            .filter(|r| r.title == title)
            .collect()
    }

    pub fn clear(&mut self) {
        self.state.history.clear();
    }
}

impl INotification for FakeNotification {
    fn show(&mut self, title: &str, message: &str) {
        self.state.history.push(NotificationRecord {
            title: title.to_string(),
            message: message.to_string(),
        });
    }
}
