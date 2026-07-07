use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
fn test_notify_sets_correct_fields() {
    let mut svc = NotificationService::new();
    let id = svc.notify("Title", "Msg", StatusLevel::Warning, 5000);
    let toast = svc
        .visible_toasts()
        .into_iter()
        .find(|t| t.id == id)
        .unwrap();
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
fn test_info_convenience() {
    let mut svc = NotificationService::new();
    svc.info("Info", "details");
    let t = &svc.visible_toasts()[0];
    assert_eq!(t.level, StatusLevel::Info);
    assert_eq!(t.title, "Info");
    assert_eq!(t.message, "details");
    assert!(t.duration_ms > 0);
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
    svc.notify("Short", "x", StatusLevel::Info, 1); // 1ms 后过期
    svc.notify("Long", "y", StatusLevel::Info, 60000); // 60s
    let visible = svc.update_at(Instant::now() + Duration::from_millis(5));
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
    let visible = svc.update_at(Instant::now() + Duration::from_millis(5));
    assert_eq!(visible.len(), 1); // 0 duration 表示不超时
}

#[test]
fn test_update_all_expired_returns_empty() {
    let mut svc = NotificationService::new();
    svc.notify("A", "", StatusLevel::Info, 1);
    svc.notify("B", "", StatusLevel::Info, 1);
    let visible = svc.update_at(Instant::now() + Duration::from_millis(5));
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
    svc.on_change(Box::new(move || {
        c.store(true, Ordering::SeqCst);
    }));
    svc.notify("A", "", StatusLevel::Info, 4000);
    assert!(called.load(Ordering::SeqCst));
}

#[test]
fn test_on_change_called_on_dismiss() {
    let mut svc = NotificationService::new();
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    svc.on_change(Box::new(move || {
        c.fetch_add(1, Ordering::SeqCst);
    }));
    let id = svc.notify("A", "", StatusLevel::Info, 4000);
    svc.dismiss(id);
    assert_eq!(count.load(Ordering::SeqCst), 2); // notify + dismiss
}

#[test]
fn test_on_change_called_on_dismiss_all() {
    let mut svc = NotificationService::new();
    let called = Arc::new(AtomicBool::new(false));
    let c = called.clone();
    svc.on_change(Box::new(move || {
        c.store(true, Ordering::SeqCst);
    }));
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
    svc.on_change(Box::new(move || {
        c.store(true, Ordering::SeqCst);
    }));
    svc.dismiss_all(); // 空队列，不应触发
    assert!(!called.load(Ordering::SeqCst));
}

// ════════════════════════════════════════════════════════════════════
// has_active / visible_toasts 边界
// ════════════════════════════════════════════════════════════════════

#[test]
fn test_has_active_false_after_dismiss_all() {
    let mut svc = NotificationService::new();
    svc.notify("A", "", StatusLevel::Info, 4000);
    svc.dismiss_all();
    assert!(!svc.has_active());
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
    impl crate::native::traits::system::INotification for SpyNotifier {
        fn show(&mut self, _title: &str, _message: &str) {
            self.called.store(true, Ordering::SeqCst);
        }
    }
    let called = Arc::new(AtomicBool::new(false));
    let notifier = SpyNotifier {
        called: called.clone(),
    };
    let mut svc = NotificationService::new()
        .with_platform(Box::new(notifier))
        .max_visible(5);
    svc.notify("A", "", StatusLevel::Info, 4000);
    assert!(called.load(Ordering::SeqCst));
}
