use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::traits::{EventHandler, WidgetAnimation};
use crate::ui::widgets::feedback::message::*;
use crate::ui::{AccessibilityRole, AnimationConfig, EventResult, Placement, SystemEvent};
use std::cell::Cell;
use std::rc::Rc;

fn render_message(message: &Message, frame: Rect, surface_size: (i32, i32)) -> String {
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
            WidgetRender::render(message, frame, ctx, &tree);
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

fn local_center(rect: Rect, frame: Rect) -> Point {
    Point::new(
        rect.x + rect.w * 0.5 - frame.x,
        rect.y + rect.h * 0.5 - frame.y,
    )
}

#[test]
fn measure_preserves_message_zero_layout_footprint() {
    let measured = Message::new().measure(Constraints::loose(Size::new(200.0, 80.0)));

    assert_eq!(measured, Size::zero());
}

#[test]
fn empty_message_does_not_block_hit_test() {
    let message = Message::new();
    let frame = Rect::new(0.0, 0.0, 800.0, 600.0);

    assert_eq!(message.hit_bounds(frame), None);
}

#[test]
fn bottom_left_message_stack_stays_inside_window() {
    let message = Message::new().placement(Placement::BottomLeft);
    message.info("first");
    message.success("second");

    let bounds = message
        .hit_bounds(Rect::new(0.0, 0.0, 800.0, 600.0))
        .expect("可见消息应有命中区域");

    assert_eq!(bounds, Rect::new(12.0, 500.0, 380.0, 88.0));
}

#[test]
fn every_message_placement_anchors_inside_window() {
    let cases = [
        (Placement::Top, Point::new(210.0, 12.0)),
        (Placement::TopLeft, Point::new(12.0, 12.0)),
        (Placement::TopRight, Point::new(408.0, 12.0)),
        (Placement::Bottom, Point::new(210.0, 548.0)),
        (Placement::BottomLeft, Point::new(12.0, 548.0)),
        (Placement::BottomRight, Point::new(408.0, 548.0)),
        (Placement::Left, Point::new(12.0, 280.0)),
        (Placement::Right, Point::new(408.0, 280.0)),
    ];

    for (placement, expected_origin) in cases {
        let message = Message::new().placement(placement);
        message.info("visible");
        let bounds = message
            .hit_bounds(Rect::new(0.0, 0.0, 800.0, 600.0))
            .expect("可见消息应有命中区域");

        assert_eq!(Point::new(bounds.x, bounds.y), expected_origin);
        assert_eq!(Size::new(bounds.w, bounds.h), Size::new(380.0, 40.0));
    }
}

#[test]
fn handle_keeps_queue_identity_and_dismisses_by_id() {
    let message = Message::new();
    let handle = message.handle();
    let id = handle.info("saved");

    assert_eq!(handle.items()[0].content, "saved");
    assert!(message.dismiss(id));
    assert!(handle.is_empty());
}

#[test]
fn message_holds_without_frames_and_expires_on_one_timer() {
    let mut message = Message::new();
    message.success("saved");

    assert!(!WidgetAnimation::update_animation(&mut message, 0.2));
    let (timer_id, delay) = EventHandler::active_timer(&message).expect("message timer");
    assert_eq!(delay, std::time::Duration::from_secs(3));

    assert_eq!(
        EventHandler::on_event(
            &mut message,
            &SystemEvent::Timer {
                id: timer_id as u32,
            },
        ),
        EventResult::Handled
    );
    assert!(message.items().is_empty());
    assert!(WidgetAnimation::update_animation(&mut message, 0.1));
    assert!(!WidgetAnimation::update_animation(&mut message, 0.05));
}

#[test]
fn close_hit_requires_matching_release_before_starting_leave() {
    let mut message = Message::new().leave_animation(AnimationConfig::fade_out(0.2));
    let id = message.add(MessageItem {
        type_: crate::native::traits::system::StatusLevel::Info,
        content: "persistent".into(),
        duration_ms: 0,
        closable: true,
    });
    WidgetAnimation::update_animation(&mut message, 0.2);
    let frame = Rect::new(100.0, 50.0, 800.0, 600.0);
    let bounds = message.hit_bounds(frame).expect("visible message");

    let close = Point::new(
        bounds.x + bounds.w - 8.0 - frame.x,
        bounds.y + bounds.h * 0.5 - frame.y,
    );
    assert_eq!(
        message.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(message.items().len(), 1, "PointerDown must not dismiss");
    assert_eq!(
        message.on_event(&pointer(
            "up",
            Point::new(bounds.x + 20.0 - frame.x, bounds.y + 20.0 - frame.y),
        )),
        EventResult::Handled
    );
    assert_eq!(message.items().len(), 1, "release outside must cancel");

    assert_eq!(
        message.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&pointer("up", close)),
        EventResult::NotHandled
    );
    assert_eq!(message.items().len(), 1, "PointerLeave must cancel");

    assert_eq!(
        message.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&SystemEvent::FocusOut),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&pointer("up", close)),
        EventResult::NotHandled
    );
    assert_eq!(message.items().len(), 1, "FocusOut must cancel");

    assert_eq!(
        message.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&pointer("up", close)),
        EventResult::Handled
    );
    assert!(
        !message.dismiss(id),
        "matching release removes the queue item once"
    );
    assert!(message.hit_bounds(frame).is_some());
    assert!(!WidgetAnimation::update_animation(&mut message, 0.2));
    assert!(message.hit_bounds(frame).is_none());
}

#[test]
fn message_action_is_measured_painted_and_invoked_only_by_a_matching_release() {
    let calls = Rc::new(Cell::new(0));
    let calls_for_action = Rc::clone(&calls);
    let mut message = Message::new()
        .action("Retry request", move || {
            calls_for_action.set(calls_for_action.get() + 1);
        })
        .icon("star")
        .leave_animation(AnimationConfig::fade_out(0.2));
    message.add(MessageItem {
        type_: StatusLevel::Info,
        content: "persistent".into(),
        duration_ms: 0,
        closable: true,
    });
    let frame = Rect::new(100.0, 50.0, 520.0, 180.0);
    let (card, action, close) = message.interaction_rects_for_test(frame)[0];
    let action = action.expect("action rect");
    let close = close.expect("close rect");
    assert!(action.w > 0.0 && action.h > 0.0);
    assert!(close.w > 0.0 && close.h > 0.0);
    assert!(action.x >= card.x && action.x + action.w <= close.x);
    assert!(close.x >= card.x && close.x + close.w <= card.x + card.w);

    let action_point = local_center(action, frame);
    let close_point = local_center(close, frame);
    assert_eq!(
        message.on_event(&pointer("down", action_point)),
        EventResult::Handled,
        "actions are interactive while an item is entering"
    );
    assert_eq!(
        message.on_event(&pointer("up", action_point)),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(message.items().len(), 1, "action must not dismiss the item");

    assert!(!WidgetAnimation::update_animation(&mut message, 0.2));
    let display = render_message(&message, frame, (720, 280));
    assert!(display.contains("Retry request"), "{display}");
    let star = crate::ui::widgets::icon::icon_char("star");
    assert!(
        display.contains(star) || display.contains("\\u{e176}"),
        "the custom Lucide icon must be painted: {display}"
    );

    assert_eq!(
        message.on_event(&pointer("down", action_point)),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&pointer("up", close_point)),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 1, "release in close must not invoke action");
    assert_eq!(message.items().len(), 1);

    assert_eq!(
        message.on_event(&pointer("down", close_point)),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&pointer("up", action_point)),
        EventResult::Handled
    );
    assert_eq!(message.items().len(), 1, "release in action must not close");

    assert_eq!(
        message.on_event(&pointer("down", action_point)),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&pointer("up", action_point)),
        EventResult::NotHandled
    );
    assert_eq!(calls.get(), 1, "PointerLeave must disarm the action");

    assert_eq!(
        message.on_event(&pointer("down", close_point)),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&pointer("up", close_point)),
        EventResult::Handled
    );
    assert!(message.items().is_empty());
    assert_eq!(
        message.on_event(&pointer("down", action_point)),
        EventResult::NotHandled,
        "a leaving item must not expose its action"
    );
    assert!(!WidgetAnimation::update_animation(&mut message, 0.2));
    assert!(message.hit_bounds(frame).is_none());
}

#[test]
fn message_action_slot_tracks_label_measurement_without_overlapping_close() {
    fn action_width(label: &str, frame: Rect) -> f32 {
        let message = Message::new().action(label, || {});
        message.add(MessageItem {
            type_: StatusLevel::Info,
            content: "content".into(),
            duration_ms: 0,
            closable: true,
        });
        let (_, action, close) = message.interaction_rects_for_test(frame)[0];
        let action = action.expect("action");
        let close = close.expect("close");
        assert!(action.x + action.w <= close.x);
        action.w
    }

    let frame = Rect::new(35.0, 70.0, 520.0, 180.0);
    let short = action_width("Go", frame);
    let long = action_width("Retry operation now", frame);
    assert!(long > short, "long labels need a wider measured slot");

    let empty = Message::new().action(" \n", || panic!("empty action must not run"));
    empty.info("content");
    assert!(empty.interaction_rects_for_test(frame)[0].1.is_none());
}

#[test]
fn message_action_release_must_match_the_same_queue_entry() {
    let calls = Rc::new(Cell::new(0));
    let calls_for_action = Rc::clone(&calls);
    let mut message = Message::new().action("Open", move || {
        calls_for_action.set(calls_for_action.get() + 1);
    });
    for content in ["first", "second"] {
        message.add(MessageItem {
            type_: StatusLevel::Info,
            content: content.into(),
            duration_ms: 0,
            closable: true,
        });
    }
    assert!(!WidgetAnimation::update_animation(&mut message, 0.2));
    let frame = Rect::new(70.0, 40.0, 520.0, 180.0);
    let controls = message.interaction_rects_for_test(frame);
    let first = local_center(controls[0].1.expect("first action"), frame);
    let second = local_center(controls[1].1.expect("second action"), frame);

    assert_eq!(
        message.on_event(&pointer("down", first)),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&pointer("up", second)),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(message.items().len(), 2);

    assert_eq!(
        message.on_event(&pointer("down", first)),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&pointer("up", first)),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn message_timer_expiry_disarms_an_armed_action_before_leave() {
    let calls = Rc::new(Cell::new(0));
    let calls_for_action = Rc::clone(&calls);
    let mut message = Message::new()
        .action("Undo", move || {
            calls_for_action.set(calls_for_action.get() + 1)
        })
        .leave_animation(AnimationConfig::fade_out(0.2));
    message.add(MessageItem {
        type_: StatusLevel::Info,
        content: "timed".into(),
        duration_ms: 25,
        closable: true,
    });
    assert!(!WidgetAnimation::update_animation(&mut message, 0.2));
    let frame = Rect::new(90.0, 40.0, 520.0, 180.0);
    let action = message.interaction_rects_for_test(frame)[0]
        .1
        .expect("action");
    let action_point = local_center(action, frame);
    let (timer_id, _) = EventHandler::active_timer(&message).expect("timer");

    assert_eq!(
        message.on_event(&pointer("down", action_point)),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&SystemEvent::Timer {
            id: timer_id as u32,
        }),
        EventResult::Handled
    );
    assert!(message.items().is_empty());
    assert_eq!(
        message.on_event(&pointer("up", action_point)),
        EventResult::NotHandled
    );
    assert_eq!(calls.get(), 0);
    assert!(!WidgetAnimation::update_animation(&mut message, 0.2));
}

#[test]
fn message_reconcile_preserves_queue_and_replaces_action_behavior() {
    let old_calls = Rc::new(Cell::new(0));
    let old_calls_for_action = Rc::clone(&old_calls);
    let mut message = Message::new().action("Old", move || {
        old_calls_for_action.set(old_calls_for_action.get() + 1);
    });
    message.add(MessageItem {
        type_: StatusLevel::Warning,
        content: "keep me".into(),
        duration_ms: 0,
        closable: true,
    });
    assert!(!WidgetAnimation::update_animation(&mut message, 0.2));
    let frame = Rect::new(80.0, 30.0, 600.0, 220.0);
    let old_action = message.interaction_rects_for_test(frame)[0]
        .1
        .expect("old action");
    let old_point = local_center(old_action, frame);
    assert_eq!(
        message.on_event(&pointer("down", old_point)),
        EventResult::Handled
    );

    let new_calls = Rc::new(Cell::new(0));
    let new_calls_for_action = Rc::clone(&new_calls);
    message.sync_from(
        Message::new()
            .action("New action", move || {
                new_calls_for_action.set(new_calls_for_action.get() + 1);
            })
            .icon("bell"),
    );
    assert_eq!(message.items()[0].content, "keep me");
    let accessibility = message.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Alert);
    assert_eq!(accessibility.name.as_deref(), Some("keep me"));
    assert_eq!(
        message.on_event(&pointer("up", old_point)),
        EventResult::NotHandled,
        "reconcile must cancel an action armed under the old configuration"
    );
    assert_eq!(old_calls.get(), 0);
    assert_eq!(new_calls.get(), 0);
    let display = render_message(&message, frame, (760, 280));
    assert!(display.contains("New action"), "{display}");
    let bell = crate::ui::widgets::icon::icon_char("bell");
    assert!(display.contains(bell) || display.contains("\\u{e059}"));

    let new_action = message.interaction_rects_for_test(frame)[0]
        .1
        .expect("new action");
    let new_point = local_center(new_action, frame);
    assert_eq!(
        message.on_event(&pointer("down", new_point)),
        EventResult::Handled
    );
    assert_eq!(
        message.on_event(&pointer("up", new_point)),
        EventResult::Handled
    );
    assert_eq!(new_calls.get(), 1);
    assert_eq!(message.items().len(), 1);
}

#[test]
fn constrained_message_stack_elides_and_keeps_visible_bounds_inside_surface() {
    let mut message = Message::new();
    for content in [
        "hidden oldest",
        "second",
        "超长中英文 mixed Message content 必须在关闭按钮前省略",
        "latest",
    ] {
        message.add(MessageItem {
            type_: StatusLevel::Info,
            content: content.to_owned(),
            duration_ms: 0,
            closable: true,
        });
    }
    assert!(!WidgetAnimation::update_animation(&mut message, 1.0));
    let frame = Rect::new(0.0, 0.0, 132.0, 90.0);
    let commands = render_message(&message, frame, (132, 90));
    let bounds = message
        .hit_bounds(frame)
        .expect("visible constrained messages");

    assert!(frame.contains(Point::new(bounds.x, bounds.y)));
    assert!(frame.contains(Point::new(bounds.x + bounds.w, bounds.y + bounds.h)));
    assert_eq!(bounds.h, 88.0, "only the latest two messages fit in 90px");
    assert!(commands.contains("PushClip { rect: Rect { x: 0.0, y: 0.0, w: 132.0, h: 90.0 } }"));
    assert!(
        commands.contains('…'),
        "long visible content must elide: {commands}"
    );
    assert!(!commands.contains("hidden oldest"));
    assert!(!commands.contains("w: -") && !commands.contains("h: -"));

    assert_eq!(
        Message::new().hit_bounds(Rect::new(f32::NAN, 0.0, -10.0, f32::INFINITY)),
        None
    );
}

#[test]
fn message_snapshot_exposes_alert_content_only_while_queue_is_nonempty() {
    let message = Message::new();
    assert_eq!(
        message.snapshot_fields().accessibility().role,
        AccessibilityRole::Generic
    );

    message.info("正在保存");
    message.error("保存失败");
    let accessibility = message.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Alert);
    assert_eq!(accessibility.name.as_deref(), Some("正在保存；保存失败"));
}

#[test]
fn slide_motion_dirty_bounds_cover_full_sweep_not_whole_window() {
    let mut message =
        Message::new().enter_animation(AnimationConfig::slide_in(Placement::Top, 0.2));
    message.info("moving");
    assert!(WidgetAnimation::update_animation(&mut message, 0.1));

    let frame = Rect::new(0.0, 0.0, 800.0, 600.0);
    let dirty = WidgetAnimation::dirty_bounds(&message, frame);

    assert!(dirty.contains(Point::new(400.0, 0.0)));
    assert!(dirty.y >= frame.y && dirty.y + dirty.h <= frame.y + frame.h);
    assert!(dirty.w < frame.w);
    assert!(dirty.h < 100.0);
}
