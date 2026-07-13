use crate::native::notification::NotificationService;
use crate::native::notification::ToastEntry;
use crate::tests::common::*;
use crate::ui::widgets::feedback::notification::*;

#[test]
fn measure_preserves_notification_zero_layout_footprint() {
    let measured = Notification::new().measure(Constraints::loose(Size::new(200.0, 80.0)));

    assert_eq!(measured, Size::zero());
}

#[test]
fn item_from_toast_entry_maps_visible_fields() {
    let toast = ToastEntry {
        id: 7,
        title: "Save failed".to_string(),
        message: "Disk is read-only".to_string(),
        level: StatusLevel::Error,
        duration_ms: 6000,
        visible: true,
        created_at: Instant::now(),
    };

    let item = NotificationItem::from_toast_entry(&toast);

    assert_eq!(item.type_, StatusLevel::Error);
    assert_eq!(item.title, "Save failed");
    assert_eq!(item.description, "Disk is read-only");
    assert_eq!(item.duration_ms, 6000);
    assert!(item.closable);
}

#[test]
fn replace_from_toasts_keeps_only_visible_toasts() {
    let notification = Notification::new();
    notification.info("Old message", "will be replaced");
    let now = Instant::now();
    let toasts = vec![
        ToastEntry {
            id: 1,
            title: "Visible".to_string(),
            message: "shown".to_string(),
            level: StatusLevel::Warning,
            duration_ms: 5000,
            visible: true,
            created_at: now,
        },
        ToastEntry {
            id: 2,
            title: "Hidden".to_string(),
            message: "skipped".to_string(),
            level: StatusLevel::Info,
            duration_ms: 4000,
            visible: false,
            created_at: now,
        },
    ];

    notification.replace_from_toasts(&toasts);
    let queue = notification.queue();
    let queue = queue.borrow();

    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].title, "Visible");
    assert_eq!(queue[0].description, "shown");
    assert_eq!(queue[0].type_, StatusLevel::Warning);
}

#[test]
fn notify_error_from_service_syncs_non_fatal_error_to_queue() {
    let notification = Notification::new();
    let mut service = NotificationService::new();

    let id = notification.notify_error_from_service(
        &mut service,
        &Error::warn(Errc::InvalidState, "cache is stale"),
    );

    assert!(id.is_some());
    let queue = notification.queue();
    let queue = queue.borrow();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].type_, StatusLevel::Warning);
    assert_eq!(queue[0].title, "Warning");
    assert!(queue[0].description.contains("cache is stale"));
}

#[test]
fn notify_result_error_from_service_keeps_ok_and_fatal_silent() {
    let notification = Notification::new();
    let mut service = NotificationService::new();
    let ok: crate::core::Result<u32> = Ok(7);
    let fatal: crate::core::Result<u32> = Err(Error::fatal(Errc::InvalidState, "must abort"));

    assert_eq!(
        notification.notify_result_error_from_service(&mut service, &ok),
        None
    );
    assert_eq!(
        notification.notify_result_error_from_service(&mut service, &fatal),
        None
    );

    assert!(notification.queue().borrow().is_empty());
    assert!(service.visible_toasts().is_empty());
}

#[test]
fn empty_notification_does_not_block_hit_test() {
    let notification = Notification::new();
    let frame = Rect::new(0.0, 0.0, 800.0, 600.0);
    let hit = notification.hit_bounds(frame).unwrap_or_default();
    assert_eq!(hit, Rect::new(0.0, 0.0, 0.0, 0.0));
}

#[test]
fn visible_notification_hit_bounds_cover_toast_stack() {
    let notification = Notification::new();
    notification.info("Saved", "done");
    let frame = Rect::new(0.0, 0.0, 800.0, 600.0);
    let hit = notification.hit_bounds(frame).expect("toast hit bounds");
    assert!(hit.w > 0.0 && hit.h > 0.0);
    assert!(hit.x >= frame.x);
    assert!(hit.y >= frame.y);
    assert!(hit.x + hit.w <= frame.x + frame.w);
    assert!(hit.y + hit.h <= frame.y + frame.h);
}
