use crate::draw::spatial::Orientation;
use crate::native::traits::system::StatusLevel;
use crate::tests::common::*;
use crate::ui::widgets::Alert;
use crate::ui::AccessibilityRole;
use crate::{
    draw::engine::cpu::pixel_surface::PixelSurface,
    draw::engine::cpu::shared_rasterizer::SharedRasterizer,
};

fn render_alert(alert: &Alert, frame: Rect, surface_size: (i32, i32)) -> String {
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
            WidgetRender::render(alert, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn pointer_down(pos: Point) -> SystemEvent {
    SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

fn pointer_up(pos: Point) -> SystemEvent {
    SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

#[test]
fn closable_alert_requires_matching_release_in_close_region() {
    let mut alert = Alert::new("network unavailable").closable();
    let body = pointer_down(Point::new(20.0, 18.0));
    let close_down = pointer_down(Point::new(280.0, 18.0));
    let close_up = pointer_up(Point::new(280.0, 18.0));

    assert_eq!(alert.on_event(&body), EventResult::NotHandled);
    assert!(alert.is_visible());
    assert_eq!(alert.on_event(&close_down), EventResult::Handled);
    assert!(alert.is_visible(), "PointerDown must not commit close");
    assert_eq!(
        alert.on_event(&pointer_up(Point::new(20.0, 18.0))),
        EventResult::Handled
    );
    assert!(alert.is_visible(), "release outside must cancel close");

    assert_eq!(alert.on_event(&close_down), EventResult::Handled);
    assert_eq!(
        alert.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(alert.on_event(&close_up), EventResult::NotHandled);
    assert!(alert.is_visible(), "PointerLeave must cancel close");

    assert_eq!(alert.on_event(&close_down), EventResult::Handled);
    alert.sync_from(Alert::new("network unavailable").closable());
    assert_eq!(alert.on_event(&close_up), EventResult::NotHandled);
    assert!(alert.is_visible(), "reconcile must cancel incomplete close");

    assert_eq!(alert.on_event(&close_down), EventResult::Handled);
    assert_eq!(alert.on_event(&close_up), EventResult::Handled);
    assert!(!alert.is_visible());
    assert!(alert.take_layout_request());
    assert!(!alert.take_layout_request());
    assert_eq!(
        alert
            .semantic_event(ComponentId::new(5), &close_up)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("closed".into())
    );
}

#[test]
fn closable_alert_is_keyboard_accessible_and_can_be_reopened() {
    let mut alert = Alert::new("saved").closable();
    let enter = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&alert), 1);
    assert_eq!(alert.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(alert.on_event(&enter), EventResult::Handled);
    assert!(
        alert.is_visible(),
        "KeyDown must show pressed state without closing"
    );
    assert_eq!(
        alert.on_event(&SystemEvent::KeyUp {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!alert.is_visible());
    assert_eq!(WidgetComponent::tab_index(&alert), 0);

    alert.open();
    assert!(alert.is_visible());
    assert!(alert.take_layout_request());
    assert_eq!(WidgetComponent::tab_index(&alert), 1);
}

#[test]
fn alert_measure_collapses_when_closed_and_accessibility_keeps_description() {
    let mut alert = Alert::new("同步失败")
        .description("请检查网络后重试")
        .closable();
    let accessibility = alert.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Alert);
    assert_eq!(accessibility.name.as_deref(), Some("同步失败"));
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("请检查网络后重试")
    );

    alert.close();
    assert_eq!(
        WidgetLayout::measure(&alert, Constraints::loose(Size::new(500.0, 500.0))),
        Size::zero()
    );
}

#[test]
fn constrained_alert_clips_and_elides_inside_normalized_actual_frame() {
    let alert = Alert::new("这是一条很长的中英文 mixed alert message")
        .description("说明文字同样不能覆盖关闭按钮或绘制到 frame 外")
        .closable();
    let display_list = render_alert(&alert, Rect::new(10.0, 8.0, 132.0, 54.0), (160, 80));

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 8.0, w: 132.0, h: 54.0 } }"),
        "all Alert paint must be clipped to its actual frame: {display_list}"
    );
    assert!(
        display_list.contains('…'),
        "long Alert text must elide: {display_list}"
    );
    assert!(!display_list.contains("w: -") && !display_list.contains("h: -"));

    let invalid = render_alert(
        &alert,
        Rect::new(f32::NAN, f32::INFINITY, -20.0, f32::NAN),
        (40, 40),
    );
    assert!(
        !invalid.contains("NaN") && !invalid.contains("inf"),
        "{invalid}"
    );
}

#[test]
fn alert_close_hover_and_press_have_distinct_visual_feedback() {
    let mut alert = Alert::new("可关闭提示").closable();
    let normal = render_alert(&alert, Rect::new(0.0, 0.0, 300.0, 36.0), (320, 56));
    assert_eq!(
        alert.on_event(&SystemEvent::PointerMove {
            pos: Point::new(282.0, 18.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let hovered = render_alert(&alert, Rect::new(0.0, 0.0, 300.0, 36.0), (320, 56));
    assert_ne!(normal, hovered, "hover must produce visible close feedback");

    assert_eq!(
        alert.on_event(&pointer_down(Point::new(282.0, 18.0))),
        EventResult::Handled
    );
    let pressed = render_alert(&alert, Rect::new(0.0, 0.0, 300.0, 36.0), (320, 56));
    assert_ne!(hovered, pressed, "pressed feedback must differ from hover");
    assert!(alert.is_visible());
}

#[test]
fn non_closable_alert_does_not_claim_close_interaction() {
    let mut alert = Alert::new("informational");

    assert_eq!(WidgetComponent::tab_index(&alert), 0);
    assert_eq!(
        alert.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Escape,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert!(alert.is_visible());
}

#[test]
fn alert_status_constructors_drive_distinct_status_snapshots_and_paint() {
    let variants = [
        (Alert::success("success"), StatusLevel::Success),
        (Alert::info("info"), StatusLevel::Info),
        (Alert::warning("warning"), StatusLevel::Warning),
        (Alert::error("error"), StatusLevel::Error),
    ];
    let mut paints = Vec::new();
    for (alert, expected) in variants {
        assert!(matches!(
            alert.snapshot_fields(),
            SnapshotFields::Alert { type_, .. } if type_ == expected
        ));
        paints.push(render_alert(
            &alert,
            Rect::new(0.0, 0.0, 300.0, 36.0),
            (320, 56),
        ));
    }
    paints.dedup();
    assert_eq!(
        paints.len(),
        4,
        "each status must use its own visual tokens/icon"
    );
}

#[test]
fn alert_action_requires_matching_release_and_is_keyboard_accessible_without_closing() {
    let calls = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let mut alert = Alert::new("new version").action("details", {
        let calls = calls.clone();
        move || calls.set(calls.get() + 1)
    });
    render_alert(&alert, Rect::new(0.0, 0.0, 300.0, 36.0), (320, 56));
    let action = Point::new(250.0, 18.0);

    assert_eq!(WidgetComponent::tab_index(&alert), 1);
    assert_eq!(alert.on_event(&pointer_down(action)), EventResult::Handled);
    assert_eq!(calls.get(), 0, "PointerDown must only arm the action");
    assert_eq!(
        alert.on_event(&pointer_up(Point::new(20.0, 18.0))),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 0, "release outside must cancel the action");

    assert_eq!(alert.on_event(&pointer_down(action)), EventResult::Handled);
    assert_eq!(alert.on_event(&pointer_up(action)), EventResult::Handled);
    assert_eq!(calls.get(), 1);
    assert!(
        alert.is_visible(),
        "actions must not implicitly dismiss the Alert"
    );
    let semantic = alert
        .semantic_event(ComponentId::new(27), &pointer_up(action))
        .expect("action semantic event");
    assert_eq!(semantic.kind, SemanticKind::Submit);
    assert_eq!(semantic.text_payload(), Some("details"));

    assert_eq!(alert.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(
        alert.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(
        alert.on_event(&SystemEvent::KeyUp {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 2);
    assert!(alert.is_visible());
}

#[test]
fn alert_action_and_close_targets_do_not_overlap_and_banner_changes_real_paint() {
    let calls = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let mut alert = Alert::warning("maintenance")
        .action("details", {
            let calls = calls.clone();
            move || calls.set(calls.get() + 1)
        })
        .closable();
    let normal = render_alert(&alert, Rect::new(0.0, 0.0, 300.0, 36.0), (320, 56));

    let action = Point::new(230.0, 18.0);
    assert_eq!(alert.on_event(&pointer_down(action)), EventResult::Handled);
    assert_eq!(alert.on_event(&pointer_up(action)), EventResult::Handled);
    assert_eq!(calls.get(), 1);
    assert!(alert.is_visible());

    let close = Point::new(282.0, 18.0);
    assert_eq!(alert.on_event(&pointer_down(close)), EventResult::Handled);
    assert_eq!(alert.on_event(&pointer_up(close)), EventResult::Handled);
    assert_eq!(calls.get(), 1, "close target must not invoke the action");
    assert!(!alert.is_visible());

    let banner = render_alert(
        &Alert::warning("maintenance")
            .action("details", || {})
            .closable()
            .banner(true),
        Rect::new(0.0, 0.0, 300.0, 36.0),
        (320, 56),
    );
    assert_ne!(
        normal, banner,
        "banner mode must alter actual container paint"
    );
    assert!(banner.contains("radius: None"));
}
