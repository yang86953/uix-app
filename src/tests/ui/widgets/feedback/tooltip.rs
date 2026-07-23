use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::view::{button, ViewAdapter, ViewNode};
use crate::ui::widgets::feedback::{Tooltip, TooltipPlacement, TriggerMode};
use crate::ui::widgets::Button;
use crate::ui::AccessibilityRole;

fn render_tooltip(tooltip: &Tooltip, frame: Rect, surface_size: (i32, i32)) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut display_list = crate::draw::command::DisplayList::new();
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
            WidgetRender::render(tooltip, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn pointer(kind: &str, pos: Point, button: MouseButton) -> SystemEvent {
    match kind {
        "down" => SystemEvent::PointerDown {
            pos,
            button,
            mods: KeyMod::NONE,
        },
        "up" => SystemEvent::PointerUp {
            pos,
            button,
            mods: KeyMod::NONE,
        },
        _ => unreachable!("unsupported pointer kind"),
    }
}

fn assert_rect_close(actual: Rect, expected: Rect) {
    assert!(
        (actual.x - expected.x).abs() < 0.001
            && (actual.y - expected.y).abs() < 0.001
            && (actual.w - expected.w).abs() < 0.001
            && (actual.h - expected.h).abs() < 0.001,
        "expected {expected:?}, got {actual:?}"
    );
}

#[test]
fn tooltip_enter_animation_finishes_visible() {
    let mut tooltip = Tooltip::new("Help");

    tooltip.open();
    assert!(tooltip.is_visible());
    assert!(tooltip.is_present());

    assert!(WidgetAnimation::update_animation(&mut tooltip, 0.05));
    assert!(!WidgetAnimation::update_animation(&mut tooltip, 1.0));
    assert!(tooltip.is_visible());
    assert!(tooltip.is_present());
}

#[test]
fn tooltip_exit_animation_stays_present_until_finished() {
    let mut tooltip = Tooltip::new("Help");
    tooltip.open();
    assert!(!WidgetAnimation::update_animation(&mut tooltip, 1.0));

    tooltip.close();
    assert!(!tooltip.is_visible());
    assert!(tooltip.is_present());

    assert!(WidgetAnimation::update_animation(&mut tooltip, 0.03));
    assert!(!WidgetAnimation::update_animation(&mut tooltip, 1.0));
    assert!(!tooltip.is_visible());
    assert!(!tooltip.is_present());
}

#[test]
fn cjk_tooltip_uses_visible_text_width_for_overlay_and_damage() {
    let frame = Rect::new(100.0, 100.0, 80.0, 28.0);
    let mut tooltip = Tooltip::new("提示文字").placement(TooltipPlacement::Top);
    tooltip.open();
    assert!(!WidgetAnimation::update_animation(&mut tooltip, 1.0));

    let overlay = WidgetRender::overlay_entry(&tooltip, ComponentId::new(7), frame)
        .expect("open tooltip overlay");
    let bubble = Rect::new(108.0, 66.0, 64.0, 26.0);
    assert_rect_close(overlay.bounds_rect().expect("tooltip bounds"), bubble);
    assert_rect_close(
        WidgetRender::dirty_rect(&tooltip, frame),
        frame.union(&bubble),
    );
}

#[test]
fn tooltip_colors_and_arrow_drive_real_paint_overlay_geometry_and_reconcile() {
    let frame = Rect::new(100.0, 100.0, 80.0, 28.0);
    let mut tooltip = Tooltip::new("Styled")
        .placement(TooltipPlacement::Top)
        .bg(Color::from_rgba(12, 34, 56, 255))
        .color(Color::from_rgba(210, 220, 230, 255))
        .arrow(true);
    tooltip.open();
    assert!(!WidgetAnimation::update_animation(&mut tooltip, 1.0));

    let arrowed = render_tooltip(&tooltip, frame, (280, 180));
    assert!(
        arrowed.contains("color: Color { r: 12, g: 34, b: 56, a: 255 }"),
        "Tooltip::bg must paint the bubble surface: {arrowed}"
    );
    assert!(
        arrowed.contains("color: Color { r: 210, g: 220, b: 230, a: 255 }"),
        "Tooltip::color must paint the text: {arrowed}"
    );
    assert!(
        arrowed.contains("FillPath"),
        "arrow paint is missing: {arrowed}"
    );
    let arrowed_bounds = WidgetRender::overlay_entry(&tooltip, ComponentId::new(7), frame)
        .expect("open Tooltip overlay")
        .bounds_rect()
        .expect("bounded Tooltip overlay");

    tooltip.sync_from(
        Tooltip::new("Styled")
            .placement(TooltipPlacement::Top)
            .trigger(TriggerMode::Click)
            .bg(Color::from_rgba(78, 90, 123, 255))
            .color(Color::from_rgba(9, 8, 7, 255))
            .arrow(false),
    );
    assert!(
        tooltip.is_visible(),
        "style reconcile preserves runtime visibility"
    );
    let arrowless = render_tooltip(&tooltip, frame, (280, 180));
    assert!(
        arrowless.contains("color: Color { r: 78, g: 90, b: 123, a: 255 }"),
        "reconcile must replace the painted background: {arrowless}"
    );
    assert!(
        arrowless.contains("color: Color { r: 9, g: 8, b: 7, a: 255 }"),
        "reconcile must replace the painted text color: {arrowless}"
    );
    assert!(
        !arrowless.contains("FillPath"),
        "arrow(false) must remove arrow geometry: {arrowless}"
    );
    let arrowless_bounds = WidgetRender::overlay_entry(&tooltip, ComponentId::new(7), frame)
        .expect("open Tooltip overlay")
        .bounds_rect()
        .expect("bounded Tooltip overlay");
    assert_eq!(arrowless_bounds.y, arrowed_bounds.y + 4.0);
    assert_eq!(arrowless_bounds.w, arrowed_bounds.w);
    assert!(matches!(
        tooltip.snapshot_fields(),
        SnapshotFields::Tooltip {
            bg_color: Some(Color {
                r: 78,
                g: 90,
                b: 123,
                a: 255
            }),
            text_color: Some(Color {
                r: 9,
                g: 8,
                b: 7,
                a: 255
            }),
            arrow: false,
            ..
        }
    ));
}

#[test]
fn tooltip_click_and_context_menu_require_matching_release_and_support_keyboard_exit() {
    let inside = Point::new(20.0, 12.0);
    let outside = Point::new(120.0, 12.0);
    let mut tooltip = Tooltip::new("Help").trigger(TriggerMode::Click);

    assert_eq!(
        tooltip.on_event(&pointer("down", inside, MouseButton::Left)),
        EventResult::Handled
    );
    assert!(!tooltip.is_visible(), "PointerDown only arms the trigger");
    assert_eq!(
        tooltip.on_event(&pointer("up", outside, MouseButton::Left)),
        EventResult::Handled
    );
    assert!(!tooltip.is_visible(), "release outside must cancel");

    tooltip.on_event(&pointer("down", inside, MouseButton::Left));
    assert_eq!(
        tooltip.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(
        tooltip.on_event(&pointer("up", inside, MouseButton::Left)),
        EventResult::NotHandled
    );
    assert!(!tooltip.is_visible(), "PointerLeave must disarm activation");

    tooltip.on_event(&pointer("down", inside, MouseButton::Left));
    tooltip.on_event(&pointer("up", inside, MouseButton::Left));
    assert!(tooltip.is_visible());
    assert_eq!(
        tooltip.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Escape,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!tooltip.is_visible());
    assert!(tooltip.is_present(), "Escape keeps the leave transition");
    assert!(!WidgetAnimation::update_animation(&mut tooltip, 1.0));

    assert_eq!(
        tooltip.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!tooltip.is_visible(), "KeyDown only arms activation");
    assert_eq!(
        tooltip.on_event(&SystemEvent::KeyUp {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(tooltip.is_visible());
    assert_eq!(
        tooltip.on_event(&SystemEvent::WindowBlur),
        EventResult::Handled
    );
    assert!(!tooltip.is_visible());

    let mut context = Tooltip::new("Menu").trigger(TriggerMode::ContextMenu);
    assert_eq!(
        context.on_event(&pointer("down", inside, MouseButton::Left)),
        EventResult::NotHandled
    );
    assert_eq!(
        context.on_event(&pointer("down", inside, MouseButton::Right)),
        EventResult::Handled
    );
    assert!(!context.is_visible());
    assert_eq!(
        context.on_event(&pointer("up", inside, MouseButton::Right)),
        EventResult::Handled
    );
    assert!(context.is_visible());

    let mut armed = Tooltip::new("Armed").trigger(TriggerMode::Click);
    armed.on_event(&pointer("down", inside, MouseButton::Left));
    armed.sync_from(Tooltip::new("Updated").trigger(TriggerMode::Hover));
    assert_eq!(
        armed.on_event(&pointer("up", inside, MouseButton::Left)),
        EventResult::NotHandled
    );
    assert!(!armed.is_visible(), "reconcile must clear stale activation");
}

#[test]
fn tooltip_wrapper_owns_hit_testing_preserves_trigger_semantics_and_focus_exit() {
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        Tooltip::new("Accessible help")
            .trigger(TriggerMode::Click)
            .arrow(false),
        vec![button("Help").into()],
    ));
    let root = tree.root_id().expect("Tooltip root");
    let frame = Rect::new(10.0, 8.0, 120.0, 32.0);
    tree.get_mut(root).expect("Tooltip root").set_frame(frame);
    tree.get_mut(root).expect("Tooltip root").set_active(true);
    tree.layout();
    let button_id = tree.find_by_type::<Button>().expect("trigger Button");
    assert_eq!(tree.get(button_id).expect("trigger Button").frame(), frame);
    let inside = Point::new(frame.x + 20.0, frame.y + 12.0);
    assert_eq!(tree.hit_test(inside), Some(root));

    assert_eq!(
        tree.dispatch_event(&pointer("down", inside, MouseButton::Left)),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&pointer("up", inside, MouseButton::Left)),
        EventResult::Handled
    );
    assert!(tree
        .get(root)
        .expect("Tooltip root")
        .component()
        .as_any()
        .downcast_ref::<Tooltip>()
        .expect("Tooltip component")
        .is_visible());
    let semantics = tree.semantic_snapshot_body();
    assert!(semantics.nodes.iter().any(|node| {
        node.accessibility.role == AccessibilityRole::Button
            && node.accessibility.name.as_deref() == Some("Help")
    }));

    tree.set_focus(Some(root));
    tree.dispatch_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&SystemEvent::KeyUp {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert!(!tree
        .get(root)
        .expect("Tooltip root")
        .component()
        .as_any()
        .downcast_ref::<Tooltip>()
        .expect("Tooltip component")
        .is_visible());

    let mut focus = Tooltip::new("Focus help").trigger(TriggerMode::Focus);
    assert_eq!(focus.on_focus_within(true), EventResult::Handled);
    assert!(focus.is_visible());
    assert_eq!(focus.on_focus_within(false), EventResult::Handled);
    assert!(!focus.is_visible());
    assert!(
        focus.is_present(),
        "focus exit retains the leave transition"
    );
}
