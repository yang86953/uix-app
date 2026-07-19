use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::native::notification::NotificationService;
use crate::native::notification::ToastEntry;
use crate::tests::common::*;
use crate::ui::traits::{EventHandler, WidgetAnimation};
use crate::ui::widgets::feedback::notification::*;
use crate::ui::{AccessibilityRole, AnimationConfig, EventResult, Placement, SystemEvent};

fn render_notification(
    notification: &Notification,
    frame: Rect,
    surface_size: (i32, i32),
) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut display_list = crate::draw::painting::DisplayList::new();
    {
        let mut ctx = PaintContext::new_for_test(
            &mut canvas,
            font,
            &fonts,
            &images,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            surface_size.0,
            surface_size.1,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(notification, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn pointer(kind: &str, pos: Point) -> SystemEvent {
    match kind {
        "down" => SystemEvent::PointerDown {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        },
        "up" => SystemEvent::PointerUp {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        },
        _ => unreachable!("unsupported pointer kind"),
    }
}

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
    let queue = notification.items();

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
    let queue = notification.items();
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

    assert!(notification.items().is_empty());
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

#[test]
fn bottom_left_notification_stack_uses_shared_placement() {
    let notification = Notification::new().placement(Placement::BottomLeft);
    notification.info("First", "with description");
    notification.success("Second", "");

    let bounds = notification
        .hit_bounds(Rect::new(0.0, 0.0, 800.0, 600.0))
        .expect("可见通知应有命中区域");

    assert_eq!(bounds, Rect::new(24.0, 462.0, 384.0, 126.0));
}

#[test]
fn every_notification_placement_anchors_inside_window() {
    let cases = [
        (Placement::Top, Point::new(208.0, 12.0)),
        (Placement::TopLeft, Point::new(24.0, 12.0)),
        (Placement::TopRight, Point::new(392.0, 12.0)),
        (Placement::Bottom, Point::new(208.0, 540.0)),
        (Placement::BottomLeft, Point::new(24.0, 540.0)),
        (Placement::BottomRight, Point::new(392.0, 540.0)),
        (Placement::Left, Point::new(24.0, 276.0)),
        (Placement::Right, Point::new(392.0, 276.0)),
    ];

    for (placement, expected_origin) in cases {
        let notification = Notification::new().placement(placement);
        notification.info("Visible", "");
        let bounds = notification
            .hit_bounds(Rect::new(0.0, 0.0, 800.0, 600.0))
            .expect("可见通知应有命中区域");

        assert_eq!(Point::new(bounds.x, bounds.y), expected_origin);
        assert_eq!(Size::new(bounds.w, bounds.h), Size::new(384.0, 48.0));
    }
}

#[test]
fn notification_holds_without_frames_and_expires_on_one_timer() {
    let mut notification = Notification::new();
    notification.info("Saved", "done");

    assert!(!WidgetAnimation::update_animation(&mut notification, 0.2));
    let (timer_id, delay) = EventHandler::active_timer(&notification).expect("notification timer");
    assert_eq!(delay, std::time::Duration::from_millis(4500));

    assert_eq!(
        EventHandler::on_event(
            &mut notification,
            &SystemEvent::Timer {
                id: timer_id as u32,
            },
        ),
        EventResult::Handled
    );
    assert!(notification.items().is_empty());
    assert!(WidgetAnimation::update_animation(&mut notification, 0.1));
    assert!(!WidgetAnimation::update_animation(&mut notification, 0.05));
}

#[test]
fn handle_dismisses_persistent_notification_after_leave() {
    let mut notification = Notification::new().leave_animation(AnimationConfig::fade_out(0.2));
    let handle = notification.handle();
    let id = handle.add(NotificationItem {
        type_: StatusLevel::Info,
        title: "Persistent".into(),
        description: "manual close".into(),
        duration_ms: 0,
        closable: true,
    });
    WidgetAnimation::update_animation(&mut notification, 0.2);

    assert_eq!(EventHandler::active_timer(&notification), None);
    assert!(handle.dismiss(id));
    assert!(handle.is_empty());
    assert!(WidgetAnimation::update_animation(&mut notification, 0.1));
    assert!(!WidgetAnimation::update_animation(&mut notification, 0.1));
}

#[test]
fn close_hit_requires_matching_release_before_starting_leave() {
    let mut notification = Notification::new().leave_animation(AnimationConfig::fade_out(0.2));
    let id = notification.add(NotificationItem {
        type_: StatusLevel::Info,
        title: "Persistent".into(),
        description: "manual close".into(),
        duration_ms: 0,
        closable: true,
    });
    assert!(!WidgetAnimation::update_animation(&mut notification, 0.2));
    let frame = Rect::new(100.0, 50.0, 520.0, 180.0);
    let bounds = notification
        .hit_bounds(frame)
        .expect("visible notification");
    let close = Point::new(
        bounds.x + bounds.w - 8.0 - frame.x,
        bounds.y + bounds.h * 0.5 - frame.y,
    );

    assert_eq!(
        notification.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(
        notification.items().len(),
        1,
        "PointerDown must not dismiss"
    );
    assert_eq!(
        notification.on_event(&pointer(
            "up",
            Point::new(bounds.x + 20.0 - frame.x, bounds.y + 20.0 - frame.y),
        )),
        EventResult::Handled
    );
    assert_eq!(notification.items().len(), 1, "release outside must cancel");

    assert_eq!(
        notification.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(
        notification.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(
        notification.on_event(&pointer("up", close)),
        EventResult::NotHandled
    );
    assert_eq!(notification.items().len(), 1, "PointerLeave must cancel");

    assert_eq!(
        notification.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(
        notification.on_event(&SystemEvent::FocusOut),
        EventResult::Handled
    );
    assert_eq!(
        notification.on_event(&pointer("up", close)),
        EventResult::NotHandled
    );
    assert_eq!(notification.items().len(), 1, "FocusOut must cancel");

    assert_eq!(
        notification.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(
        notification.on_event(&pointer("up", close)),
        EventResult::Handled
    );
    assert!(
        !notification.dismiss(id),
        "matching release removes the queue item once"
    );
    assert!(notification.hit_bounds(frame).is_some());
    assert!(!WidgetAnimation::update_animation(&mut notification, 0.2));
    assert!(notification.hit_bounds(frame).is_none());
}

#[test]
fn constrained_notification_stack_elides_and_keeps_visible_bounds_inside_surface() {
    let mut notification = Notification::new();
    for (title, description) in [
        ("hidden oldest", "hidden body"),
        ("second hidden", "hidden body"),
        (
            "超长中英文 mixed Notification title 必须在关闭按钮前省略",
            "超长说明 description 也必须在卡片边界内省略",
        ),
        ("latest", ""),
    ] {
        notification.add(NotificationItem {
            type_: StatusLevel::Info,
            title: title.to_owned(),
            description: description.to_owned(),
            duration_ms: 0,
            closable: true,
        });
    }
    assert!(!WidgetAnimation::update_animation(&mut notification, 1.0));
    let frame = Rect::new(0.0, 0.0, 132.0, 132.0);
    let commands = render_notification(&notification, frame, (132, 132));
    let bounds = notification
        .hit_bounds(frame)
        .expect("visible constrained notifications");

    assert!(frame.contains(Point::new(bounds.x, bounds.y)));
    assert!(frame.contains(Point::new(bounds.x + bounds.w, bounds.y + bounds.h)));
    assert_eq!(bounds.h, 126.0, "only the latest two notifications fit");
    assert!(commands.contains("PushClip { rect: Rect { x: 0.0, y: 0.0, w: 132.0, h: 132.0 } }"));
    assert!(
        commands.contains('…'),
        "visible text must elide: {commands}"
    );
    assert!(!commands.contains("hidden oldest"));
    assert!(!commands.contains("second hidden"));
    assert!(!commands.contains("w: -") && !commands.contains("h: -"));
    assert_eq!(
        Notification::new().hit_bounds(Rect::new(f32::NAN, 0.0, -10.0, f32::INFINITY)),
        None
    );
}

#[test]
fn notification_snapshot_exposes_alert_content_only_while_queue_is_nonempty() {
    let notification = Notification::new();
    assert_eq!(
        notification.snapshot_fields().accessibility().role,
        AccessibilityRole::Generic
    );

    notification.info("正在保存", "请稍候");
    notification.error("保存失败", "磁盘只读");
    let accessibility = notification.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Alert);
    assert_eq!(accessibility.name.as_deref(), Some("正在保存；保存失败"));
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("请稍候；磁盘只读")
    );
}

#[test]
fn slide_motion_dirty_bounds_cover_only_notification_sweep() {
    let mut notification =
        Notification::new().enter_animation(AnimationConfig::slide_in(Placement::Right, 0.2));
    notification.info("Moving", "narrow damage");
    notification.hit_bounds(Rect::new(0.0, 0.0, 800.0, 600.0));
    assert!(WidgetAnimation::update_animation(&mut notification, 0.1));

    let dirty = WidgetAnimation::dirty_bounds(&notification, Rect::new(0.0, 0.0, 800.0, 600.0));

    assert!(dirty.contains(Point::new(796.0, 40.0)));
    assert!(dirty.w < 450.0);
    assert!(dirty.h < 100.0);
}
