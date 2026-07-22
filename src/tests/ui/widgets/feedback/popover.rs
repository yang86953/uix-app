use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::traits::{EventHandler, WidgetAnimation, WidgetComponent, WidgetRender};
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::{Label, Popover, PopoverPlacement, PopoverTrigger};
use crate::ui::{AccessibilityRole, AnimationConfig, LayoutChild, Placement, State};

fn render_popover(popover: &Popover, frame: Rect, surface_size: (i32, i32)) -> String {
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
            WidgetRender::render(popover, frame, ctx, &tree);
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
fn popover_is_focusable_and_keyboard_toggles_click_trigger() {
    let mut popover = Popover::new("Details");

    assert_eq!(WidgetComponent::tab_index(&popover), 1);
    assert_eq!(
        popover.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    assert_eq!(
        popover.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!popover.is_visible(), "key down only arms activation");
    assert_eq!(
        popover.on_event(&SystemEvent::KeyUp {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(popover.is_visible());
    assert_eq!(
        popover.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Escape,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!popover.is_visible());
    assert!(popover.is_present(), "exit animation remains present");
}

#[test]
fn popover_uses_custom_enter_and_leave_durations() {
    let mut popover = Popover::new("details")
        .enter_animation(AnimationConfig::fade_in(0.4))
        .leave_animation(AnimationConfig::fade_out(0.3));

    popover.open();
    assert!(WidgetAnimation::update_animation(&mut popover, 0.2));
    assert!(!WidgetAnimation::update_animation(&mut popover, 0.2));

    popover.close();
    assert!(WidgetAnimation::update_animation(&mut popover, 0.2));
    assert!(popover.is_present());
    assert!(!WidgetAnimation::update_animation(&mut popover, 0.1));
    assert!(!popover.is_present());
}

#[test]
fn slide_animation_dirty_rect_covers_the_full_motion_sweep() {
    let frame = Rect::new(300.0, 200.0, 80.0, 28.0);
    let mut popover = Popover::new("details")
        .placement(PopoverPlacement::Top)
        .enter_animation(AnimationConfig::slide_in(Placement::Right, 0.2));
    popover.open();

    let dirty = WidgetRender::dirty_rect(&popover, frame);

    assert_eq!(dirty.x, 288.0);
    assert!((dirty.x + dirty.w - 556.0).abs() < 1e-4);
    assert!(dirty.contains(Point::new(555.0, 150.0)));
}

#[test]
fn popover_overlay_is_removed_only_after_custom_leave_finishes() {
    let mut popover = Popover::new("details").leave_animation(AnimationConfig::fade_out(0.3));
    popover.open();
    assert!(!WidgetAnimation::update_animation(&mut popover, 1.0));

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(popover));
    tree.get_mut(id)
        .expect("popover root")
        .set_frame(Rect::new(300.0, 200.0, 80.0, 28.0));
    tree.get_mut(id).expect("popover root").set_active(true);
    tree.rebuild_widget_overlays();
    assert_eq!(tree.overlay_stack().len(), 1);

    tree.get_mut(id)
        .expect("popover root")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Popover>()
        .expect("popover component")
        .close();

    assert!(tree.update(0.2));
    assert_eq!(tree.overlay_stack().len(), 1);
    assert!(!tree.update(0.1));
    assert!(tree.overlay_stack().is_empty());
}

#[test]
fn animation_discovery_registers_overlay_before_enter_finishes() {
    let popover = Popover::new("details").enter_animation(AnimationConfig::fade_in(0.3));
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(popover));
    tree.get_mut(id)
        .expect("popover root")
        .set_frame(Rect::new(300.0, 200.0, 80.0, 28.0));
    tree.get_mut(id).expect("popover root").set_active(true);
    tree.rebuild_widget_overlays();
    assert!(tree.overlay_stack().is_empty());

    tree.get_mut(id)
        .expect("popover root")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Popover>()
        .expect("popover component")
        .open();

    assert!(tree.update(0.1));
    assert_eq!(tree.overlay_stack().len(), 1);
}

#[test]
fn click_popover_requires_matching_release_and_exposes_expanded_content() {
    let mut popover = Popover::new("完整说明内容").title("质量详情");
    let inside = Point::new(20.0, 12.0);
    let outside = Point::new(120.0, 12.0);

    assert_eq!(
        popover.on_event(&pointer_down(inside)),
        EventResult::Handled
    );
    assert!(!popover.is_visible());
    assert_eq!(popover.on_event(&pointer_up(outside)), EventResult::Handled);
    assert!(!popover.is_visible());

    assert_eq!(
        popover.on_event(&pointer_down(inside)),
        EventResult::Handled
    );
    assert_eq!(
        popover.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(popover.on_event(&pointer_up(inside)), EventResult::Handled);
    assert!(!popover.is_visible());

    assert_eq!(
        popover.on_event(&pointer_down(inside)),
        EventResult::Handled
    );
    assert_eq!(popover.on_event(&pointer_up(inside)), EventResult::Handled);
    assert!(popover.is_visible());
    let accessibility = popover.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.name.as_deref(), Some("质量详情"));
    assert_eq!(accessibility.state.expanded, Some(true));
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("完整说明内容")
    );
}

#[test]
fn constrained_popover_flips_clips_elides_and_lays_out_trigger_child() {
    let mut popover = Popover::new(
        "超长中英文 mixed popover content that must remain inside the constrained surface",
    )
    .title("超长质量详情标题 mixed title")
    .placement(PopoverPlacement::Top);
    popover.open();
    let frame = Rect::new(100.0, 4.0, 80.0, 28.0);
    let commands = render_popover(&popover, frame, (132, 120));

    assert!(commands.contains("PushClip { rect: Rect { x: 0.0, y: 0.0, w: 132.0, h: 120.0 } }"));
    assert!(commands.contains('…'));
    assert!(!commands.contains("mixed popover content that must remain"));
    assert!(!commands.contains("w: -") && !commands.contains("h: -"));
    let overlay = WidgetRender::overlay_entry(&popover, ComponentId::new(7), frame)
        .expect("open popover overlay");
    let bounds = overlay.bounds_rect().expect("bounded overlay");
    assert!(Rect::new(0.0, 0.0, 132.0, 120.0).contains(Point::new(
        bounds.x + bounds.w - 0.01,
        bounds.y + bounds.h - 0.01,
    )));

    let child = ComponentId::new(9);
    assert_eq!(
        popover.layout_children(
            frame,
            &[LayoutChild::new(child, Size::new(20.0, 10.0))],
            &WidgetTree::new(),
        ),
        vec![(child, frame)]
    );
}

#[test]
fn popover_custom_trigger_view_materializes_as_a_real_child() {
    let build = |content: &str, trigger: &str| {
        ViewNode::leaf(
            Popover::new(content).trigger_view(crate::ui::view::label(trigger.to_owned())),
        )
    };
    let mut tree = ViewAdapter::build_nodes(build("details", "打开"));
    let root = tree.root_id().expect("popover root");
    let frame = Rect::new(12.0, 8.0, 120.0, 32.0);
    tree.get_mut(root).expect("popover root").set_frame(frame);
    tree.get_mut(root).expect("popover root").set_active(true);
    tree.layout();

    let label_id = tree.find_by_type::<Label>().expect("custom trigger label");
    let label = tree
        .get(label_id)
        .expect("custom trigger node")
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .expect("Label component");
    assert_eq!(label.text(), "打开");
    assert_eq!(tree.get(label_id).expect("trigger node").frame(), frame);
    let inside = Point::new(frame.x + 20.0, frame.y + 12.0);
    assert_eq!(
        tree.dispatch_event(&pointer_down(inside)),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&pointer_up(inside)),
        EventResult::Handled
    );
    let popover = tree
        .get(root)
        .expect("popover root")
        .component()
        .as_any()
        .downcast_ref::<Popover>()
        .expect("Popover component");
    assert!(popover.is_visible());
    let accessibility = popover.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.state.expanded, Some(true));

    ViewAdapter::reconcile_nodes(&mut tree, build("updated details", "新版触发器"));
    assert_eq!(tree.root_id(), Some(root));
    let reconciled_label_id = tree
        .find_by_type::<Label>()
        .expect("reconciled custom trigger label");
    let label = tree
        .get(reconciled_label_id)
        .expect("reconciled custom trigger node")
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .expect("Label component");
    assert_eq!(reconciled_label_id, label_id);
    assert_eq!(label.text(), "新版触发器");
    assert!(tree
        .get(root)
        .expect("popover root")
        .component()
        .as_any()
        .downcast_ref::<Popover>()
        .expect("Popover component")
        .is_visible());
}

#[test]
fn popover_background_and_arrow_drive_real_paint_reconcile_and_bounded_overlay() {
    let first_background = Color::from_rgba(12, 34, 56, 255);
    let second_background = Color::from_rgba(78, 90, 123, 255);
    let frame = Rect::new(100.0, 40.0, 80.0, 28.0);
    let mut popover = Popover::new("before")
        .title("Title")
        .placement(PopoverPlacement::Bottom)
        .bg(first_background)
        .arrow(true);
    popover.open();
    assert!(!WidgetAnimation::update_animation(&mut popover, 1.0));

    let arrowed = render_popover(&popover, frame, (240, 180));
    assert!(
        arrowed.contains("color: Color { r: 12, g: 34, b: 56, a: 255 }"),
        "Popover::bg must paint the popup surface: {arrowed}"
    );
    assert!(
        arrowed.contains("FillPath"),
        "arrow(true) must emit arrow geometry: {arrowed}"
    );

    popover.sync_from(
        Popover::new("after")
            .title("Updated")
            .placement(PopoverPlacement::Bottom)
            .bg(second_background)
            .arrow(false),
    );
    assert!(
        popover.is_visible(),
        "style reconcile preserves runtime open state"
    );
    let arrowless = render_popover(&popover, frame, (240, 180));
    assert!(
        arrowless.contains("color: Color { r: 78, g: 90, b: 123, a: 255 }"),
        "reconcile must replace the painted popup background: {arrowless}"
    );
    assert!(
        !arrowless.contains("FillPath"),
        "arrow(false) must remove arrow paint: {arrowless}"
    );
    let overlay = WidgetRender::overlay_entry(&popover, ComponentId::new(44), frame)
        .expect("open Popover overlay");
    let bounds = overlay.bounds_rect().expect("bounded Popover overlay");
    let surface = Rect::new(0.0, 0.0, 240.0, 180.0);
    assert!(surface.contains(Point::new(bounds.x, bounds.y)));
    assert!(surface.contains(Point::new(
        bounds.x + bounds.w - 0.01,
        bounds.y + bounds.h - 0.01,
    )));
    assert!(matches!(
        popover.snapshot_fields(),
        SnapshotFields::Popover {
            content,
            arrow: false,
            visible: true,
            ..
        } if content == "after"
    ));
    let accessibility = popover.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.state.expanded, Some(true));
    assert_eq!(accessibility.state.value_text.as_deref(), Some("after"));
}

#[test]
fn controlled_popover_writes_click_hover_and_focus_changes_without_feedback_loops() {
    let click_open = State::new(false);
    let mut click = Popover::new("details").controlled_open(&click_open);
    let inside = Point::new(20.0, 12.0);

    assert_eq!(click.on_event(&pointer_down(inside)), EventResult::Handled);
    assert_eq!(click.on_event(&pointer_up(inside)), EventResult::Handled);
    assert!(click.is_visible());
    assert!(click_open.get());
    let open_generation = click_open.generation();

    click.open();
    assert_eq!(
        click_open.generation(),
        open_generation,
        "writing the already-controlled value must not create a reconcile loop"
    );
    assert_eq!(
        click.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Escape,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!click.is_visible());
    assert!(!click_open.get());

    let hover_open = State::new(false);
    let mut hover = Popover::new("hover")
        .trigger(PopoverTrigger::Hover)
        .controlled_open(&hover_open);
    assert_eq!(
        hover.on_event(&SystemEvent::PointerEnter),
        EventResult::Handled
    );
    assert!(hover.is_visible());
    assert!(hover_open.get());
    assert_eq!(
        hover.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert!(!hover.is_visible());
    assert!(!hover_open.get());

    let focus_open = State::new(false);
    let mut focus = Popover::new("focus")
        .trigger(PopoverTrigger::Focus)
        .controlled_open(&focus_open);
    assert_eq!(focus.on_focus_within(true), EventResult::Handled);
    assert!(focus.is_visible());
    assert!(focus_open.get());
    assert_eq!(focus.on_focus_within(false), EventResult::Handled);
    assert!(!focus.is_visible());
    assert!(!focus_open.get());
}

#[test]
fn external_controlled_popover_state_requests_reconcile_and_drives_the_live_instance() {
    let open = State::new(false);
    let build = || {
        ViewAdapter::capture_root(|| ViewNode::leaf(Popover::new("details").controlled_open(&open)))
    };
    let mut tree = ViewAdapter::build_nodes(build());
    let root = tree.root_id().expect("popover root");
    tree.reset_invalidation();

    open.set(true);
    assert!(tree.take_reconcile_requested());
    ViewAdapter::reconcile_nodes(&mut tree, build());
    let popover = tree
        .get(root)
        .expect("reused popover")
        .component()
        .as_any()
        .downcast_ref::<Popover>()
        .expect("Popover component");
    assert!(popover.is_visible());
    assert_eq!(
        popover.snapshot_fields().accessibility().state.expanded,
        Some(true)
    );

    tree.reset_invalidation();
    open.set(false);
    assert!(tree.take_reconcile_requested());
    ViewAdapter::reconcile_nodes(&mut tree, build());
    let popover = tree
        .get(root)
        .expect("reused popover")
        .component()
        .as_any()
        .downcast_ref::<Popover>()
        .expect("Popover component");
    assert!(!popover.is_visible());
    assert!(
        popover.is_present(),
        "controlled close keeps its leave transition"
    );
    assert_eq!(
        popover.snapshot_fields().accessibility().state.expanded,
        Some(false)
    );
}

#[test]
fn controlled_popover_reconcile_replaces_the_bound_state_without_reviving_the_old_one() {
    let old = State::new(true);
    let current = State::new(false);
    let mut popover = Popover::new("details").controlled_open(&old);
    assert!(popover.is_visible());

    popover.sync_from(Popover::new("details").controlled_open(&current));
    assert!(!popover.is_visible());

    old.set(false);
    old.set(true);
    assert!(
        !popover.is_visible(),
        "the replaced State must no longer control the live component"
    );

    current.set(true);
    assert_eq!(
        popover.on_event(&SystemEvent::PointerMove {
            pos: Point::new(120.0, 12.0),
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert!(popover.is_visible());
}
