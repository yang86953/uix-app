// ============================================================================
// platform/notification.rs — 消息通知服务
//
// 职责：
//   1. 包装平台 INotification（系统通知）
//   2. 管理应用内通知队列（Toast 数据源）
//   3. 提供回调注册，通知 UI 层刷新
//   4. 支持通知级别：Info / Success / Warning / Error
//
// 原位于 services crate，迁入 platform 层以消除服务层。
// ============================================================================

use std::collections::VecDeque;
use crate::StatusLevel;

/// 通知级别（统一使用 uix_platform::StatusLevel）。
pub use crate::StatusLevel as NotificationLevel;

/// 单条通知条目，作为 Toast 组件的数据源。
#[derive(Debug, Clone)]
pub struct ToastEntry {
    pub id: u64,
    pub title: String,
    pub message: String,
    pub level: StatusLevel,
    /// 持续时间（毫秒）。0 = 手动关闭。None = 使用默认值。
    pub duration_ms: u32,
    /// 是否当前可见（未被关闭）。
    pub visible: bool,
    /// 创建时间戳。
    pub created_at: std::time::Instant,
}

/// 通知服务 — 管理系统通知和应用内 Toast。
///
/// 通过 DI 注入使用：
/// ```ignore
/// let mut svc = NotificationService::new();
/// svc.notify("Hello", "World", StatusLevel::Info, 4000);
/// ```
pub struct NotificationService {
    /// 平台通知实现（可选 — headless 模式下为 None）。
    platform_notifier: Option<Box<dyn crate::INotification>>,
    /// 应用内 Toast 队列。
    toasts: VecDeque<ToastEntry>,
    /// 同时可见的最大 Toast 数。
    max_visible: usize,
    /// 自增 ID 计数器。
    next_id: u64,
    /// 可选回调：Toast 队列变化时触发（UI 刷新）。
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

    /// 附加平台通知后端（如 Platform trait 提供）。
    pub fn with_platform(mut self, notifier: Box<dyn crate::INotification>) -> Self {
        self.platform_notifier = Some(notifier);
        self
    }

    /// 设置同时可见的最大 Toast 数。
    pub fn max_visible(mut self, n: usize) -> Self {
        self.max_visible = n.max(1);
        self
    }

    /// 更新同时可见的最大 Toast 数（原地）。
    pub fn set_max_visible(&mut self, n: usize) {
        self.max_visible = n.max(1);
    }

    /// 注册 Toast 队列变化时的回调。
    pub fn on_change(&mut self, cb: Box<dyn Fn() + Send>) {
        self.on_change = Some(cb);
    }

    // ── 核心 API ─────────────────────────────────────────────────────────

    const DURATION_INFO: u32 = 4000;
    const DURATION_SUCCESS: u32 = 4000;
    const DURATION_WARNING: u32 = 5000;
    const DURATION_ERROR: u32 = 6000;

    /// 发送默认通知（Info, 4s）。
    pub fn info(&mut self, title: &str, message: &str) {
        self.notify(title, message, StatusLevel::Info, Self::DURATION_INFO);
    }

    /// 发送成功通知。
    pub fn success(&mut self, title: &str, message: &str) {
        self.notify(title, message, StatusLevel::Success, Self::DURATION_SUCCESS);
    }

    /// 发送警告通知。
    pub fn warning(&mut self, title: &str, message: &str) {
        self.notify(title, message, StatusLevel::Warning, Self::DURATION_WARNING);
    }

    /// 发送错误通知。
    pub fn error(&mut self, title: &str, message: &str) {
        self.notify(title, message, StatusLevel::Error, Self::DURATION_ERROR);
    }

    /// 发送通知（完整控制）。
    pub fn notify(
        &mut self,
        title: &str,
        message: &str,
        level: StatusLevel,
        duration_ms: u32,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        // 1. 发送平台通知（系统级弹窗/气球提示）
        if let Some(ref mut pn) = self.platform_notifier {
            pn.show(title, message);
        }

        // 2. 入队应用内 Toast
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

        // 3. 通知 UI 层
        if let Some(ref cb) = self.on_change {
            cb();
        }

        crate::log::info_fn(format!("[Notification] {:?}: {} — {}", level, title, message));

        id
    }

    /// 关闭指定 ID 的 Toast（从队列中移除）。
    pub fn dismiss(&mut self, id: u64) {
        if let Some(pos) = self.toasts.iter().position(|t| t.id == id && t.visible) {
            self.toasts.remove(pos);
            if let Some(ref cb) = self.on_change {
                cb();
            }
        }
    }

    /// 关闭所有可见 Toast（清空队列）。
    pub fn dismiss_all(&mut self) {
        if self.has_active() {
            self.toasts.clear();
            if let Some(ref cb) = self.on_change {
                cb();
            }
        }
    }

    /// 移除过期 Toast（超时自动清理），返回当前可见列表。
    pub fn update(&mut self) -> Vec<ToastEntry> {
        let now = std::time::Instant::now();
        self.toasts.retain(|t| {
            if t.duration_ms > 0 && now.duration_since(t.created_at).as_millis() as u32 >= t.duration_ms {
                return false;
            }
            true
        });
        self.toasts.iter().cloned().collect()
    }

    /// 获取当前可见的 Toast 列表。
    pub fn visible_toasts(&self) -> Vec<ToastEntry> {
        self.toasts.iter().filter(|t| t.visible).cloned().collect()
    }

    /// 检查是否有活跃的 Toast。
    pub fn has_active(&self) -> bool {
        self.toasts.iter().any(|t| t.visible)
    }

    /// 移除超出 max_visible 数量的最旧 Toast。
    fn trim_excess(&mut self) {
        let visible_count = self.toasts.iter().filter(|t| t.visible).count();
        let excess = visible_count.saturating_sub(self.max_visible);
        if excess == 0 {
            return;
        }
        let mut removed = 0usize;
        self.toasts.retain(|t| {
            if removed < excess && t.visible {
                removed += 1;
                false
            } else {
                true
            }
        });
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 测试
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    // ════════════════════════════════════════════════════════════════════
    // 基础 notify / dismiss
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn test_notify_adds_toast() {
        let mut svc = NotificationService::new();
        let id = svc.notify("Test", "Message", StatusLevel::Info, 4000);
        assert!(svc.has_active());
        assert_eq!(svc.visible_toasts().len(), 1);
        assert_eq!(svc.visible_toasts()[0].id, id);
    }

    #[test]
    fn test_notify_returns_incremented_ids() {
        let mut svc = NotificationService::new();
        let id1 = svc.notify("A", "1", StatusLevel::Info, 1000);
        let id2 = svc.notify("B", "2", StatusLevel::Info, 1000);
        assert!(id2 > id1);
    }

    #[test]
    fn test_notify_sets_correct_fields() {
        let mut svc = NotificationService::new();
        let id = svc.notify("Title", "Msg", StatusLevel::Warning, 5000);
        let toast = svc.visible_toasts().into_iter().find(|t| t.id == id).unwrap();
        assert_eq!(toast.title, "Title");
        assert_eq!(toast.message, "Msg");
        assert_eq!(toast.level, StatusLevel::Warning);
        assert_eq!(toast.duration_ms, 5000);
        assert!(toast.visible);
    }

    #[test]
    fn test_dismiss_removes_toast() {
        let mut svc = NotificationService::new();
        let id = svc.notify("Test", "Message", StatusLevel::Info, 4000);
        svc.dismiss(id);
        assert!(!svc.has_active());
        assert!(svc.visible_toasts().is_empty());
    }

    #[test]
    fn test_dismiss_unknown_id_does_nothing() {
        let mut svc = NotificationService::new();
        svc.notify("A", "1", StatusLevel::Info, 4000);
        svc.dismiss(9999);
        assert!(svc.has_active());
    }

    #[test]
    fn test_dismiss_twice_is_harmless() {
        let mut svc = NotificationService::new();
        let id = svc.notify("A", "1", StatusLevel::Info, 4000);
        svc.dismiss(id);
        svc.dismiss(id); // 第二次不应 panic
        assert!(!svc.has_active());
    }

    // ════════════════════════════════════════════════════════════════════
    // dismiss_all
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn test_dismiss_all_removes_all() {
        let mut svc = NotificationService::new();
        svc.notify("A", "1", StatusLevel::Info, 4000);
        svc.notify("B", "2", StatusLevel::Info, 4000);
        svc.notify("C", "3", StatusLevel::Info, 4000);
        svc.dismiss_all();
        assert!(!svc.has_active());
        assert!(svc.visible_toasts().is_empty());
    }

    #[test]
    fn test_dismiss_all_empty_does_nothing() {
        let mut svc = NotificationService::new();
        svc.dismiss_all(); // 不应 panic
        assert!(!svc.has_active());
    }

    // ════════════════════════════════════════════════════════════════════
    // 便利方法：info / success / warning / error
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn test_info_convenience() {
        let mut svc = NotificationService::new();
        svc.info("Info", "details");
        let t = &svc.visible_toasts()[0];
        assert_eq!(t.level, StatusLevel::Info);
        assert_eq!(t.title, "Info");
        assert_eq!(t.message, "details");
        assert!(t.duration_ms > 0);
    }

    #[test]
    fn test_success_convenience() {
        let mut svc = NotificationService::new();
        svc.success("OK", "done");
        assert_eq!(svc.visible_toasts()[0].level, StatusLevel::Success);
    }

    #[test]
    fn test_warning_convenience() {
        let mut svc = NotificationService::new();
        svc.warning("Caution", "be careful");
        assert_eq!(svc.visible_toasts()[0].level, StatusLevel::Warning);
    }

    #[test]
    fn test_error_convenience() {
        let mut svc = NotificationService::new();
        svc.error("Fail", "something broke");
        assert_eq!(svc.visible_toasts()[0].level, StatusLevel::Error);
    }

    // ════════════════════════════════════════════════════════════════════
    // max_visible / trim_excess
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn test_max_visible_trims_oldest() {
        let mut svc = NotificationService::new();
        svc.set_max_visible(3);
        svc.notify("A", "1", StatusLevel::Info, 4000);
        svc.notify("B", "2", StatusLevel::Info, 4000);
        svc.notify("C", "3", StatusLevel::Info, 4000);
        svc.notify("D", "4", StatusLevel::Info, 4000);
        svc.notify("E", "5", StatusLevel::Info, 4000);
        assert_eq!(svc.visible_toasts().len(), 3);
        // A 和 B 应被移除（最旧的两个）
        let visible = svc.visible_toasts();
        let titles: Vec<&str> = visible.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, vec!["C", "D", "E"]);
    }

    #[test]
    fn test_max_visible_no_trim_when_under_limit() {
        let mut svc = NotificationService::new();
        svc.set_max_visible(10);
        for i in 0..5 {
            svc.notify(&format!("T{}", i), "", StatusLevel::Info, 4000);
        }
        assert_eq!(svc.visible_toasts().len(), 5);
    }

    #[test]
    fn test_max_visible_minimum_is_one() {
        let mut svc = NotificationService::new();
        svc.set_max_visible(0); // 应该被 clamp 到 1
        svc.notify("A", "", StatusLevel::Info, 4000);
        svc.notify("B", "", StatusLevel::Info, 4000);
        assert_eq!(svc.visible_toasts().len(), 1);
    }

    #[test]
    fn test_max_visible_fluent_builder() {
        let mut svc = NotificationService::new().max_visible(2);
        // max_visible 是 builder 模式，set_max_visible 才是原地
        // 此处验证 builder 生效
        svc.notify("A", "", StatusLevel::Info, 4000);
        svc.notify("B", "", StatusLevel::Info, 4000);
        svc.notify("C", "", StatusLevel::Info, 4000);
        assert_eq!(svc.visible_toasts().len(), 2);
    }

    // ════════════════════════════════════════════════════════════════════
    // update — 超时自动移除
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn test_update_removes_expired() {
        let mut svc = NotificationService::new();
        svc.notify("Short", "x", StatusLevel::Info, 1);   // 1ms 后过期
        svc.notify("Long", "y", StatusLevel::Info, 60000); // 60s
        std::thread::sleep(std::time::Duration::from_millis(5));
        let visible = svc.update();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].title, "Long");
    }

    #[test]
    fn test_update_keeps_visible_when_not_expired() {
        let mut svc = NotificationService::new();
        svc.notify("A", "", StatusLevel::Info, 60000);
        svc.notify("B", "", StatusLevel::Info, 60000);
        let visible = svc.update();
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn test_update_removes_zero_duration_immediately() {
        let mut svc = NotificationService::new();
        svc.notify("A", "", StatusLevel::Info, 0); // 0 = 不自动过期（由手动关闭）
        std::thread::sleep(std::time::Duration::from_millis(5));
        let visible = svc.update();
        assert_eq!(visible.len(), 1); // 0 duration 表示不超时
    }

    #[test]
    fn test_update_all_expired_returns_empty() {
        let mut svc = NotificationService::new();
        svc.notify("A", "", StatusLevel::Info, 1);
        svc.notify("B", "", StatusLevel::Info, 1);
        std::thread::sleep(std::time::Duration::from_millis(5));
        let visible = svc.update();
        assert!(visible.is_empty());
    }

    // ════════════════════════════════════════════════════════════════════
    // on_change 回调
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn test_on_change_called_on_notify() {
        let mut svc = NotificationService::new();
        let called = Arc::new(AtomicBool::new(false));
        let c = called.clone();
        svc.on_change(Box::new(move || { c.store(true, Ordering::SeqCst); }));
        svc.notify("A", "", StatusLevel::Info, 4000);
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_on_change_called_on_dismiss() {
        let mut svc = NotificationService::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        svc.on_change(Box::new(move || { c.fetch_add(1, Ordering::SeqCst); }));
        let id = svc.notify("A", "", StatusLevel::Info, 4000);
        svc.dismiss(id);
        assert_eq!(count.load(Ordering::SeqCst), 2); // notify + dismiss
    }

    #[test]
    fn test_on_change_called_on_dismiss_all() {
        let mut svc = NotificationService::new();
        let called = Arc::new(AtomicBool::new(false));
        let c = called.clone();
        svc.on_change(Box::new(move || { c.store(true, Ordering::SeqCst); }));
        svc.notify("A", "", StatusLevel::Info, 4000);
        called.store(false, Ordering::SeqCst); // reset
        svc.dismiss_all();
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_on_change_not_called_on_empty_dismiss_all() {
        let mut svc = NotificationService::new();
        let called = Arc::new(AtomicBool::new(false));
        let c = called.clone();
        svc.on_change(Box::new(move || { c.store(true, Ordering::SeqCst); }));
        svc.dismiss_all(); // 空队列，不应触发
        assert!(!called.load(Ordering::SeqCst));
    }

    // ════════════════════════════════════════════════════════════════════
    // has_active / visible_toasts 边界
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn test_has_active_false_initially() {
        let svc = NotificationService::new();
        assert!(!svc.has_active());
    }

    #[test]
    fn test_has_active_false_after_dismiss_all() {
        let mut svc = NotificationService::new();
        svc.notify("A", "", StatusLevel::Info, 4000);
        svc.dismiss_all();
        assert!(!svc.has_active());
    }

    #[test]
    fn test_visible_toasts_empty_initially() {
        let svc = NotificationService::new();
        assert!(svc.visible_toasts().is_empty());
    }

    #[test]
    fn test_dismissed_toast_not_in_visible() {
        let mut svc = NotificationService::new();
        let id = svc.notify("A", "", StatusLevel::Info, 4000);
        svc.dismiss(id);
        assert!(svc.visible_toasts().iter().all(|t| t.id != id));
    }

    // ════════════════════════════════════════════════════════════════════
    // with_platform
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn test_with_platform_delegates_system_notification() {
        struct SpyNotifier {
            called: Arc<AtomicBool>,
        }
        impl crate::INotification for SpyNotifier {
            fn show(&mut self, _title: &str, _message: &str) {
                self.called.store(true, Ordering::SeqCst);
            }
        }
        let called = Arc::new(AtomicBool::new(false));
        let notifier = SpyNotifier { called: called.clone() };
        let mut svc = NotificationService::new()
            .with_platform(Box::new(notifier))
            .max_visible(5);
        svc.notify("A", "", StatusLevel::Info, 4000);
        assert!(called.load(Ordering::SeqCst));
    }

    // ════════════════════════════════════════════════════════════════════
    // ToastEntry 默认值和字段
    // ════════════════════════════════════════════════════════════════════

    #[test]
    fn test_toast_entry_debug_and_clone() {
        let mut svc = NotificationService::new();
        svc.notify("A", "msg", StatusLevel::Info, 4000);
        let entry = svc.visible_toasts()[0].clone();
        let debug = format!("{:?}", entry);
        assert!(debug.contains("ToastEntry"));
    }
}
