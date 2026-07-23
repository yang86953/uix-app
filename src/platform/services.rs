//! 可由独立 [`super::Platform`] 同步完成的平台服务值。

/// OS-known user directory。
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialDir {
    Home,
    AppData,
    LocalAppData,
    Documents,
    Desktop,
    Downloads,
}

/// 一条系统通知。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemNotification {
    title: String,
    message: String,
}

impl SystemNotification {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}
