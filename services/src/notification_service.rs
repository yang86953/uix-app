// ============================================================================
// services/notification_service.rs — 消息通知服务
// ============================================================================
//
// 职责：
//   1. 包装平台 INotification（系统通知）
//   2. 管理应用内通知队列（Toast 数据源）
//   3. 提供回调注册，通知 UI 层刷新
//   4. 支持通知级别：Info / Success / Warning / Error
// ============================================================================

use std::collections::VecDeque;

/// 通知级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NotificationLevel {
    Info = 0,
    Success = 1,
    Warning = 2,
    Error = 3,
}

/// A single notification entry displayed as a Toast in the UI.
#[derive(Debug, Clone)]
pub struct ToastEntry {
    pub id: u64,
    pub title: String,
    pub message: String,
    pub level: NotificationLevel,
    /// Duration in ms. 0 = manual dismiss. None = use default.
    pub duration_ms: u32,
    /// Whether the toast is currently visible (not yet dismissed).
    pub visible: bool,
    /// When the toast was created (Instant::now ticks).
    pub created_at: std::time::Instant,
}

/// Notification service — manages platform notifications and in-app toasts.
///
/// Usage via DI:
/// ```ignore
/// let svc = NotificationService::new();
/// svc.notify("Hello", "World", NotificationLevel::Info, 4000);
/// ```
pub struct NotificationService {
    /// Platform notification implementation (optional — may be None in headless mode).
    platform_notifier: Option<Box<dyn uix_platform::INotification>>,
    /// In-app toast queue (displayed as Toast widgets).
    toasts: VecDeque<ToastEntry>,
    /// Maximum number of visible toasts at once.
    max_visible: usize,
    /// Auto-incrementing ID counter.
    next_id: u64,
    /// Optional callback: invoked when the toast queue changes (UI refresh).
    on_change: Option<Box<dyn Fn() + Send>>,
}

impl Default for NotificationService {
    fn default() -> Self {
        Self {
            platform_notifier: None,
            toasts: VecDeque::new(),
            max_visible: 5,
            next_id: 0,
            on_change: None,
        }
    }
}

impl NotificationService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach a platform notification backend (e.g. from Platform trait).
    pub fn with_platform(mut self, notifier: Box<dyn uix_platform::INotification>) -> Self {
        self.platform_notifier = Some(notifier);
        self
    }

    /// Set the maximum number of simultaneously visible toasts.
    pub fn max_visible(mut self, n: usize) -> Self {
        self.max_visible = n.max(1);
        self
    }

    /// Register a callback invoked whenever the toast queue changes.
    pub fn on_change(&mut self, cb: Box<dyn Fn() + Send>) {
        self.on_change = Some(cb);
    }

    // ── Core API ─────────────────────────────────────────────────────────

    /// Send a notification with default options (Info, 4s).
    pub fn info(&mut self, title: &str, message: &str) {
        self.notify(title, message, NotificationLevel::Info, 4000);
    }

    /// Send a success notification.
    pub fn success(&mut self, title: &str, message: &str) {
        self.notify(title, message, NotificationLevel::Success, 4000);
    }

    /// Send a warning notification.
    pub fn warning(&mut self, title: &str, message: &str) {
        self.notify(title, message, NotificationLevel::Warning, 5000);
    }

    /// Send an error notification.
    pub fn error(&mut self, title: &str, message: &str) {
        self.notify(title, message, NotificationLevel::Error, 6000);
    }

    /// Send a notification with full control.
    pub fn notify(
        &mut self,
        title: &str,
        message: &str,
        level: NotificationLevel,
        duration_ms: u32,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        // 1. Send platform notification (system-level toast/balloon)
        if let Some(ref mut pn) = self.platform_notifier {
            pn.show(title, message);
        }

        // 2. Enqueue in-app toast
        let entry = ToastEntry {
            id,
            title: title.to_string(),
            message: message.to_string(),
            level,
            duration_ms,
            visible: true,
            created_at: std::time::Instant::now(),
        };
        self.toasts.push_back(entry);
        self.trim_excess();

        // 3. Notify UI layer
        if let Some(ref cb) = self.on_change {
            cb();
        }

        log::info!(
            "[Notification] {:?}: {} — {}",
            level,
            title,
            message
        );

        id
    }

    /// Dismiss a specific toast by ID.
    pub fn dismiss(&mut self, id: u64) {
        if let Some(pos) = self.toasts.iter().position(|t| t.id == id && t.visible) {
            self.toasts[pos].visible = false;
            if let Some(ref cb) = self.on_change {
                cb();
            }
        }
    }

    /// Dismiss all visible toasts.
    pub fn dismiss_all(&mut self) {
        for toast in &mut self.toasts {
            toast.visible = false;
        }
        if let Some(ref cb) = self.on_change {
            cb();
        }
    }

    /// Remove expired toasts (duration exceeded) and return updated visible list.
    pub fn update(&mut self) -> Vec<ToastEntry> {
        let now = std::time::Instant::now();
        self.toasts.retain(|t| {
            if !t.visible {
                return false;
            }
            if t.duration_ms > 0 && now.duration_since(t.created_at).as_millis() as u32 >= t.duration_ms {
                return false;
            }
            true
        });
        self.toasts.iter().cloned().collect()
    }

    /// Get currently visible toasts.
    pub fn visible_toasts(&self) -> Vec<ToastEntry> {
        self.toasts
            .iter()
            .filter(|t| t.visible)
            .cloned()
            .collect()
    }

    /// Check if there are any active toasts.
    pub fn has_active(&self) -> bool {
        self.toasts.iter().any(|t| t.visible)
    }

    /// Remove the oldest toasts if exceeding max_visible.
    fn trim_excess(&mut self) {
        let visible_count = self.toasts.iter().filter(|t| t.visible).count();
        let excess = visible_count.saturating_sub(self.max_visible);
        for _ in 0..excess {
            if let Some(pos) = self.toasts.iter().position(|t| t.visible) {
                self.toasts[pos].visible = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notify_adds_toast() {
        let mut svc = NotificationService::new();
        let id = svc.notify("Test", "Message", NotificationLevel::Info, 4000);
        assert!(svc.has_active());
        assert_eq!(svc.visible_toasts().len(), 1);
        assert_eq!(svc.visible_toasts()[0].id, id);
    }

    #[test]
    fn test_dismiss_removes_toast() {
        let mut svc = NotificationService::new();
        let id = svc.notify("Test", "Message", NotificationLevel::Info, 4000);
        svc.dismiss(id);
        assert!(!svc.has_active());
    }

    #[test]
    fn test_max_visible_trims() {
        let mut svc = NotificationService::new();
        svc.max_visible = 3;
        for i in 0..5 {
            svc.notify(&format!("Title {}", i), "Msg", NotificationLevel::Info, 4000);
        }
        assert_eq!(svc.visible_toasts().len(), 3);
    }
}
