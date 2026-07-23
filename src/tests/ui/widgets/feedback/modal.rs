use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::draw::scene::ScenePaint;
use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::feedback::modal::*;
use crate::ui::{AccessibilityRole, AnimationConfig, ClickEvent};

fn render_modal(modal: &Modal, frame: Rect, surface_size: (i32, i32)) -> String {
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
            WidgetRender::render(modal, frame, ctx, &tree);
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
fn closed_modal_trigger_opens_via_widget_tree_matching_release() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Modal::new("Dialog").overlay(true)));
    tree.get_mut(id)
        .expect("modal root")
        .set_frame(Rect::new(0.0, 0.0, 96.0, 32.0));
    let closed_tree_version = tree.tree_version();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(48.0, 16.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(
        !tree
            .get(id)
            .expect("modal root")
            .component()
            .as_any()
            .downcast_ref::<Modal>()
            .expect("modal component")
            .is_present(),
        "PointerDown must not open Modal"
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: Point::new(48.0, 16.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(tree
        .get(id)
        .expect("modal root")
        .component()
        .as_any()
        .downcast_ref::<Modal>()
        .expect("modal component")
        .is_present());
    assert!(tree
        .overlay_stack()
        .iter()
        .any(|entry| { entry.owner() == id && entry.kind() == crate::ui::OverlayKind::Modal }));
    assert!(ScenePaint::node_is_overlay(&tree, id));
    assert!(
        tree.tree_version() > closed_tree_version,
        "opening an overlay must rebuild the compositor layer tree"
    );
    assert!(
        tree.invalidation()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .dirty_region()
            .full_frame
    );
}

#[test]
fn completed_modal_exit_removes_its_overlay_presentation() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Modal::new("Dialog").visible(true).overlay(true)));
    tree.get_mut(id)
        .expect("modal root")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.get_mut(id).expect("modal root").set_active(true);
    tree.layout();
    let open_tree_version = tree.tree_version();
    assert!(tree.overlay_stack().top().is_some());

    tree.get_mut(id)
        .expect("modal root")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Modal>()
        .expect("modal component")
        .close();
    assert!(!tree.update(1.0));

    assert!(tree.overlay_stack().is_empty());
    assert!(tree.tree_version() > open_tree_version);
}

#[test]
fn modal_confirm_dispatches_each_callback_once_and_closes_context() {
    use crate::ui::view::ViewAdapter;
    use crate::ui::widgets::Button;

    let ok_count = Rc::new(Cell::new(0));
    let cancel_count = Rc::new(Cell::new(0));
    let ok_count_for_callback = ok_count.clone();
    let cancel_count_for_callback = cancel_count.clone();
    let mut tree = ViewAdapter::build(crate::ui::widgets::Modal::confirm(
        "Confirm",
        "Continue?",
        move || ok_count_for_callback.set(ok_count_for_callback.get() + 1),
        move || cancel_count_for_callback.set(cancel_count_for_callback.get() + 1),
    ));

    let buttons = tree.find_all_by_type::<Button>();
    assert_eq!(
        buttons.len(),
        2,
        "confirm keeps separate cancel and OK actions"
    );
    let ok_id = buttons
        .iter()
        .find(|(_, button)| button.text() == "确定")
        .map(|(id, _)| *id)
        .expect("confirm action button");
    let cancel_id = buttons
        .iter()
        .find(|(_, button)| button.text() == "取消")
        .map(|(id, _)| *id)
        .expect("cancel action button");
    let click = ClickEvent {
        button: MouseButton::Left,
        pos: Point::new(8.0, 8.0),
        modifiers: KeyMod::NONE,
    };

    assert_eq!(
        tree.dispatch_semantic(SemanticEvent::click(ok_id, click)),
        EventResult::Handled
    );
    assert_eq!(ok_count.get(), 1);
    assert_eq!(cancel_count.get(), 0);
    let modal = tree
        .root_id()
        .and_then(|id| tree.get(id))
        .and_then(|node| node.component().as_any().downcast_ref::<Modal>())
        .expect("confirm modal");
    assert!(!modal.is_visible());
    assert!(
        modal.is_present(),
        "OK starts the existing leave transition"
    );

    assert_eq!(
        tree.dispatch_semantic(SemanticEvent::click(ok_id, click)),
        EventResult::Handled
    );
    assert_eq!(ok_count.get(), 1, "FnOnce callback must not run twice");

    assert_eq!(
        tree.dispatch_semantic(SemanticEvent::click(cancel_id, click)),
        EventResult::Handled
    );
    assert_eq!(
        cancel_count.get(),
        0,
        "a completed confirmation must reject the stale sibling action"
    );

    let second_ok_count = Rc::new(Cell::new(0));
    let second_cancel_count = Rc::new(Cell::new(0));
    let ok_for_callback = second_ok_count.clone();
    let cancel_for_callback = second_cancel_count.clone();
    let mut cancel_tree = ViewAdapter::build(crate::ui::widgets::Modal::confirm(
        "Confirm",
        "Continue?",
        move || ok_for_callback.set(ok_for_callback.get() + 1),
        move || cancel_for_callback.set(cancel_for_callback.get() + 1),
    ));
    let buttons = cancel_tree.find_all_by_type::<Button>();
    let cancel_id = buttons
        .iter()
        .find(|(_, button)| button.text() == "取消")
        .map(|(id, _)| *id)
        .expect("cancel action button");
    let ok_id = buttons
        .iter()
        .find(|(_, button)| button.text() == "确定")
        .map(|(id, _)| *id)
        .expect("confirm action button");
    assert_eq!(
        cancel_tree.dispatch_semantic(SemanticEvent::click(cancel_id, click)),
        EventResult::Handled
    );
    assert_eq!(
        cancel_tree.dispatch_semantic(SemanticEvent::click(cancel_id, click)),
        EventResult::Handled
    );
    assert_eq!(
        cancel_tree.dispatch_semantic(SemanticEvent::click(ok_id, click)),
        EventResult::Handled
    );
    assert_eq!(second_cancel_count.get(), 1);
    assert_eq!(second_ok_count.get(), 0);
}

#[test]
fn modal_shortcuts_build_distinct_accessible_status_icons_and_one_ok_button() {
    use crate::ui::view::ViewAdapter;
    use crate::ui::widgets::{Button, Icon};

    for (builder, expected_icon, expected_status) in [
        (Modal::info("Status", "Message"), "info", "信息"),
        (
            Modal::warning("Status", "Message"),
            "alert-triangle",
            "警告",
        ),
        (Modal::error("Status", "Message"), "x-circle", "错误"),
    ] {
        let mut tree = ViewAdapter::build(builder);
        let root = tree.root_id().expect("modal root");
        tree.get_mut(root)
            .expect("modal node")
            .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
        tree.get_mut(root).expect("modal node").set_active(true);
        tree.layout();
        let buttons = tree.find_all_by_type::<Button>();
        assert_eq!(buttons.len(), 1, "shortcut dialogs expose one action");
        assert_eq!(buttons[0].1.text(), "确定");

        let icons = tree.find_all_by_type::<Icon>();
        assert_eq!(icons.len(), 1, "shortcut dialogs expose one status icon");
        let icon_frame = tree.get(icons[0].0).expect("status icon node").frame();
        assert!(
            icon_frame.w > 0.0 && icon_frame.h > 0.0,
            "the shortcut status icon must own real paint geometry"
        );
        let icon_fields = tree
            .get(icons[0].0)
            .expect("status icon node")
            .component()
            .snapshot_fields();
        assert!(matches!(
            icon_fields,
            SnapshotFields::Icon { name, .. } if name == expected_icon
        ));

        let semantics = tree.semantic_snapshot_body();
        assert!(semantics.nodes.iter().any(|node| {
            node.accessibility.role == AccessibilityRole::Image
                && node.accessibility.name.as_deref() == Some(expected_status)
        }));
        assert!(matches!(
            tree.get(tree.root_id().expect("modal root"))
                .expect("modal node")
                .component()
                .snapshot_fields(),
            SnapshotFields::Modal {
                open: true,
                footer_visible: false,
                overlay: true,
                ..
            }
        ));
    }
}

#[test]
fn modal_shortcut_ok_closes_with_pointer_and_keyboard_activation() {
    use crate::ui::view::ViewAdapter;
    use crate::ui::widgets::Button;

    fn layout_shortcut(tree: &mut WidgetTree) -> (ComponentId, ComponentId, Point) {
        let modal = tree.root_id().expect("modal root");
        tree.get_mut(modal)
            .expect("modal node")
            .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
        tree.get_mut(modal).expect("modal node").set_active(true);
        tree.layout();
        let button = tree.find_all_by_type::<Button>()[0].0;
        let frame = tree.get(button).expect("OK button").frame();
        assert!(frame.w > 0.0 && frame.h > 0.0);
        (
            modal,
            button,
            Point::new(frame.x + frame.w * 0.5, frame.y + frame.h * 0.5),
        )
    }

    let mut pointer_tree = ViewAdapter::build(Modal::info("Status", "Message"));
    let (pointer_modal, _, click) = layout_shortcut(&mut pointer_tree);
    assert_eq!(
        pointer_tree.dispatch_event(&pointer("down", click)),
        EventResult::Handled
    );
    assert!(pointer_tree
        .get(pointer_modal)
        .expect("modal")
        .component()
        .as_any()
        .downcast_ref::<Modal>()
        .expect("Modal component")
        .is_visible());
    assert_eq!(
        pointer_tree.dispatch_event(&pointer("up", click)),
        EventResult::Handled
    );
    assert!(!pointer_tree
        .get(pointer_modal)
        .expect("modal")
        .component()
        .as_any()
        .downcast_ref::<Modal>()
        .expect("Modal component")
        .is_visible());

    let mut keyboard_tree = ViewAdapter::build(
        Modal::warning("Status", "Message")
            .closable(false)
            .mask_closable(false),
    );
    let (keyboard_modal, button, _) = layout_shortcut(&mut keyboard_tree);
    keyboard_tree.set_focus(Some(button));
    assert_eq!(
        keyboard_tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(keyboard_tree
        .get(keyboard_modal)
        .expect("modal")
        .component()
        .as_any()
        .downcast_ref::<Modal>()
        .expect("Modal component")
        .is_visible());
    assert_eq!(
        keyboard_tree.dispatch_event(&SystemEvent::KeyUp {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!keyboard_tree
        .get(keyboard_modal)
        .expect("modal")
        .component()
        .as_any()
        .downcast_ref::<Modal>()
        .expect("Modal component")
        .is_visible());
}

#[test]
fn modal_shortcut_reconcile_reuses_nodes_and_replaces_status_and_close_context() {
    use crate::ui::view::ViewAdapter;
    use crate::ui::widgets::{Button, Icon};

    let mut tree = ViewAdapter::build(Modal::info("Status", "Before"));
    let root = tree.root_id().expect("modal root");
    let icon = tree.find_all_by_type::<Icon>()[0].0;

    ViewAdapter::reconcile(&mut tree, Modal::error("Status", "After"));

    assert_eq!(tree.root_id(), Some(root));
    let reconciled_icon = tree.find_all_by_type::<Icon>()[0].0;
    assert_eq!(reconciled_icon, icon);
    assert!(matches!(
        tree.get(reconciled_icon)
            .expect("reconciled status icon")
            .component()
            .snapshot_fields(),
        SnapshotFields::Icon { name, .. } if name == "x-circle"
    ));
    let button = tree.find_all_by_type::<Button>()[0].0;
    assert_eq!(
        tree.dispatch_semantic(SemanticEvent::click(
            button,
            ClickEvent {
                button: MouseButton::Left,
                pos: Point::zero(),
                modifiers: KeyMod::NONE,
            },
        )),
        EventResult::Handled
    );
    assert!(!tree
        .get(root)
        .expect("modal")
        .component()
        .as_any()
        .downcast_ref::<Modal>()
        .expect("Modal component")
        .is_visible());
}

#[test]
fn destroy_on_close_removes_modal_subtree_after_exit_transition() {
    use crate::ui::view::{button, ViewAdapter, ViewNode};

    let mut tree = ViewAdapter::build(ViewNode::new(
        Modal::new("Dialog")
            .visible(true)
            .overlay(true)
            .destroy_on_close(false),
        vec![button("Cancel").into()],
    ));
    let modal_id = tree.root_id().expect("modal root");
    tree.get_mut(modal_id)
        .expect("modal root")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.get_mut(modal_id).expect("modal root").set_active(true);
    tree.layout();

    let modal = tree
        .get_mut(modal_id)
        .expect("modal root")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Modal>()
        .expect("modal component");
    modal.sync_from(Modal::new("Dialog").overlay(true).destroy_on_close(true));
    assert!(modal.should_destroy_on_close());
    modal.close();
    assert!(!tree.update(1.0));
    assert!(tree.get(modal_id).is_none());
}

#[test]
fn closable_false_hides_and_disarms_the_close_slot_but_escape_still_exits() {
    let frame = Rect::new(24.0, 18.0, 132.0, 90.0);
    let close = Point::new(114.0, 18.0);
    let mut modal = Modal::new("Dialog")
        .size(120.0, 84.0)
        .mask_closable(false)
        .visible(true);
    assert!(!WidgetAnimation::update_animation(&mut modal, 1.0));
    let closable = render_modal(&modal, frame, (180, 128));
    let close_glyph = crate::ui::widgets::icon::icon_char("x");
    assert!(
        closable.contains(close_glyph) || closable.contains("\\u{e1b2}"),
        "the default Modal must paint a Lucide close icon: {closable}"
    );

    assert_eq!(
        modal.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    modal.sync_from(
        Modal::new("Dialog")
            .size(120.0, 84.0)
            .closable(false)
            .mask_closable(false),
    );
    assert_eq!(modal.on_event(&pointer("up", close)), EventResult::Handled);
    assert!(
        modal.is_visible(),
        "reconcile must disarm the old close slot"
    );
    assert!(matches!(
        modal.snapshot_fields(),
        SnapshotFields::Modal {
            closable: false,
            open: true,
            ..
        }
    ));

    let hidden = render_modal(&modal, frame, (180, 128));
    assert!(
        !hidden.contains(close_glyph) && !hidden.contains("\\u{e1b2}"),
        "closable(false) must remove the close icon: {hidden}"
    );
    assert_eq!(
        modal.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(modal.on_event(&pointer("up", close)), EventResult::Handled);
    assert!(modal.is_visible(), "the removed close slot must not hit");
    assert_eq!(
        modal.snapshot_fields().accessibility().role,
        AccessibilityRole::Dialog
    );

    assert_eq!(
        modal.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Escape,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!modal.is_visible());
    assert!(modal.is_present(), "Escape keeps the leave transition");
}

#[test]
fn modal_advertises_animation_capability() {
    let modal = Modal::new("Dialog").visible(true);

    assert!(modal.capabilities().contains(WidgetCapabilities::ANIMATION));
    assert!(modal.as_animation().is_some());
}

#[test]
fn modal_default_enter_transition_is_immediately_at_rest() {
    let mut modal = Modal::new("Dialog").visible(true);

    assert!(modal.transition.finished);
    assert_eq!(modal.transition.opacity_progress, 1.0);
    assert_eq!(modal.transition.scale, 1.0);
    assert!(!WidgetAnimation::update_animation(&mut modal, 0.05));
}

#[test]
fn modal_enter_transition_advances_and_marks_paint_dirty() {
    let mut modal = Modal::new("Dialog")
        .enter_animation(AnimationConfig::zoom_in(0.2))
        .visible(true);
    let initial_opacity = modal.transition.opacity_progress;

    assert!(WidgetAnimation::update_animation(&mut modal, 0.05));
    assert!(modal.transition.opacity_progress > initial_opacity);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(
        Modal::new("Dialog")
            .enter_animation(AnimationConfig::zoom_in(0.2))
            .visible(true),
    ));
    tree.get_mut(id)
        .expect("modal root")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.get_mut(id).expect("modal root").set_active(true);
    tree.invalidation().lock().unwrap().clear();

    assert!(tree.update(1.0 / 60.0));

    let queue = tree.invalidation().lock().unwrap();
    assert!(queue.has_paint_or_composite());
    assert!(queue.node_needs_paint(id));
}

#[test]
fn modal_close_finishes_exit_transition_before_internal_hide() {
    let mut modal = Modal::new("Dialog").visible(true);
    assert!(!WidgetAnimation::update_animation(&mut modal, 1.0));
    assert!(modal.is_visible());

    modal.close();
    assert!(!modal.is_visible());
    assert!(modal.is_present());
    assert!(modal.take_layout_request());

    assert!(!WidgetAnimation::update_animation(&mut modal, 1.0));
    assert!(!modal.is_present());
    assert!(modal.transition_dirty);
    assert!(
        modal.take_layout_request(),
        "finishing the overlay leave transition must restore the closed trigger slot"
    );
}

#[test]
fn closed_modal_hides_its_retained_child_subtree() {
    use crate::ui::view::{button, ViewAdapter, ViewNode};

    let mut tree = ViewAdapter::build(ViewNode::new(
        Modal::new("Dialog").visible(true).overlay(true),
        vec![button("Cancel").into()],
    ));
    let modal = tree.root_id().expect("modal root");
    tree.get_mut(modal)
        .expect("modal node")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.get_mut(modal).expect("modal node").set_active(true);
    tree.layout();
    assert!(!tree.update(1.0));
    tree.layout();

    let child = tree
        .get(modal)
        .expect("modal node")
        .children()
        .first()
        .copied()
        .expect("modal child");
    assert!(tree.visible_rect_for(child).is_some());

    tree.get_mut(modal)
        .expect("modal node")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Modal>()
        .expect("Modal component")
        .close();
    assert!(!tree.update(1.0));

    assert!(
        tree.visible_rect_for(child).is_none(),
        "a closed Modal must not expose stale child geometry"
    );
    assert_ne!(
        tree.hit_test(Point::new(300.0, 250.0)),
        Some(child),
        "a closed Modal child must not remain hittable"
    );

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(400.0, 16.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: Point::new(400.0, 16.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    tree.layout();
    assert!(!tree.update(1.0));
    tree.layout();
    assert!(
        tree.visible_rect_for(child).is_some(),
        "reopening a Modal must relayout and reveal its retained child subtree"
    );
}

#[test]
fn modal_uses_custom_enter_and_leave_animation_durations() {
    let mut modal = Modal::new("Dialog")
        .enter_animation(AnimationConfig::fade_in(0.4))
        .leave_animation(AnimationConfig::fade_out(0.3))
        .visible(true);

    assert_eq!(modal.transition.scale, 1.0);
    assert!(WidgetAnimation::update_animation(&mut modal, 0.2));
    assert!(modal.transition.opacity_progress > 0.0);
    assert!(modal.transition.opacity_progress < 1.0);

    modal.close();
    assert!(WidgetAnimation::update_animation(&mut modal, 0.2));
    assert!(modal.is_present());
    assert!(!WidgetAnimation::update_animation(&mut modal, 0.1));
    assert!(!modal.is_present());
}

#[test]
fn modal_show_builds_interactive_view_and_context_close_starts_exit() {
    use crate::ui::view::{button, ViewAdapter};
    use crate::ui::widgets::Button;

    let mut tree = ViewAdapter::build(
        Modal::show(|ctx| button("关闭").on_click_fn(move || ctx.close()))
            .title("提示")
            .width(400.0)
            .height(240.0)
            .enter_animation(AnimationConfig::fade_in(0.2))
            .leave_animation(AnimationConfig::fade_out(0.15)),
    );
    let modal_id = tree.root_id().expect("modal root");
    tree.get_mut(modal_id)
        .expect("modal node")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.get_mut(modal_id).expect("modal node").set_active(true);
    tree.layout();
    assert!(!tree.update(1.0));
    tree.layout();

    let child_id = tree
        .get(modal_id)
        .expect("modal node")
        .children()
        .first()
        .copied()
        .expect("modal content child");
    let child = tree.get(child_id).expect("modal content node");
    assert!(child.component().as_any().is::<Button>());
    let child_frame = child.frame();
    assert!(child_frame.w > 0.0 && child_frame.h > 0.0);
    assert!(matches!(
        tree.get(modal_id)
            .expect("modal node")
            .component()
            .snapshot_fields(),
        SnapshotFields::Modal {
            title,
            width: 400.0,
            height: 240.0,
            footer_visible: false,
            overlay: true,
            ..
        } if title == "提示"
    ));

    let click = Point::new(
        child_frame.x + child_frame.w * 0.5,
        child_frame.y + child_frame.h * 0.5,
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: click,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: click,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );

    let modal = tree
        .get(modal_id)
        .expect("modal node")
        .component()
        .as_any()
        .downcast_ref::<Modal>()
        .expect("Modal component");
    assert!(!modal.is_visible());
    assert!(modal.is_present());
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .needs_full_frame());
}

#[test]
fn modal_trigger_and_close_require_matching_release_in_nonzero_frame() {
    let frame = Rect::new(24.0, 18.0, 132.0, 90.0);
    let trigger = Point::new(66.0, 16.0);
    let mut modal = Modal::new("Dialog").size(120.0, 84.0);
    render_modal(&modal, frame, (180, 128));

    assert_eq!(
        modal.on_event(&pointer("down", trigger)),
        EventResult::Handled
    );
    assert!(!modal.is_present(), "PointerDown must not open Modal");
    assert_eq!(
        modal.on_event(&pointer("up", Point::new(130.0, 80.0))),
        EventResult::Handled
    );
    assert!(!modal.is_present(), "release outside must cancel trigger");

    assert_eq!(
        modal.on_event(&pointer("down", trigger)),
        EventResult::Handled
    );
    assert_eq!(
        modal.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(
        modal.on_event(&pointer("up", trigger)),
        EventResult::NotHandled
    );
    assert!(!modal.is_present(), "PointerLeave must cancel trigger");

    assert_eq!(
        modal.on_event(&pointer("down", trigger)),
        EventResult::Handled
    );
    assert_eq!(modal.on_event(&SystemEvent::FocusOut), EventResult::Handled);
    assert_eq!(
        modal.on_event(&pointer("up", trigger)),
        EventResult::NotHandled
    );
    assert!(!modal.is_present(), "FocusOut must cancel trigger");

    assert_eq!(
        modal.on_event(&pointer("down", trigger)),
        EventResult::Handled
    );
    assert_eq!(
        modal.on_event(&pointer("up", trigger)),
        EventResult::Handled
    );
    assert!(modal.is_visible());
    assert!(!WidgetAnimation::update_animation(&mut modal, 1.0));
    render_modal(&modal, frame, (180, 128));

    // The 120x84 dialog is centered inside the actual 132x90 frame. Its close
    // slot occupies the top-right 48px and must use the same release contract.
    let close = Point::new(114.0, 18.0);
    assert_eq!(
        modal.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert!(modal.is_visible(), "PointerDown must not close Modal");
    assert_eq!(
        modal.on_event(&pointer("up", Point::new(16.0, 76.0))),
        EventResult::Handled
    );
    assert!(modal.is_visible(), "release outside must cancel close");

    assert_eq!(
        modal.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(
        modal.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(modal.on_event(&pointer("up", close)), EventResult::Handled);
    assert!(modal.is_visible(), "PointerLeave must cancel close");

    assert_eq!(
        modal.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(modal.on_event(&SystemEvent::FocusOut), EventResult::Handled);
    assert_eq!(modal.on_event(&pointer("up", close)), EventResult::Handled);
    assert!(modal.is_visible(), "FocusOut must cancel close");

    assert_eq!(
        modal.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(modal.on_event(&pointer("up", close)), EventResult::Handled);
    assert!(!modal.is_visible());
    assert!(modal.is_present(), "leave transition must remain present");
}

#[test]
fn constrained_modal_normalizes_geometry_and_clips_its_child_to_the_body() {
    use crate::ui::view::{button, ViewAdapter, ViewNode};

    let frame = Rect::new(10.0, 8.0, 132.0, 90.0);
    let mut modal = Modal::new("Very long mixed Modal title that must elide")
        .size(500.0, 500.0)
        .visible(true);
    assert!(!WidgetAnimation::update_animation(&mut modal, 1.0));
    let display_list = render_modal(&modal, frame, (160, 112));
    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 8.0, w: 132.0, h: 90.0 } }"),
        "Modal paint must be clipped to its actual frame: {display_list}"
    );
    assert!(
        display_list.contains('…'),
        "long Modal title must elide before the close slot: {display_list}"
    );
    assert!(!display_list.contains("w: -") && !display_list.contains("h: -"));

    let mut tree = ViewAdapter::build(ViewNode::new(
        Modal::new("Dialog").size(500.0, 500.0).visible(true),
        vec![button("Content").into()],
    ));
    let modal_id = tree.root_id().expect("modal root");
    tree.get_mut(modal_id).expect("modal node").set_frame(frame);
    tree.get_mut(modal_id).expect("modal node").set_active(true);
    tree.layout();
    assert!(!tree.update(1.0));
    tree.layout();

    let child_id = tree
        .get(modal_id)
        .expect("modal node")
        .children()
        .first()
        .copied()
        .expect("modal child");
    let child_frame = tree.get(child_id).expect("modal child node").frame();
    for value in [child_frame.x, child_frame.y, child_frame.w, child_frame.h] {
        assert!(
            value.is_finite(),
            "child geometry must be finite: {child_frame:?}"
        );
    }
    assert!(child_frame.w >= 0.0 && child_frame.h >= 0.0);
    let modal = tree
        .get(modal_id)
        .expect("modal node")
        .component()
        .as_any()
        .downcast_ref::<Modal>()
        .expect("Modal component");
    let child_clip = WidgetRender::children_clip(modal, frame).expect("open Modal body clip");
    for value in [child_clip.x, child_clip.y, child_clip.w, child_clip.h] {
        assert!(
            value.is_finite(),
            "child clip must be finite: {child_clip:?}"
        );
    }
    assert!(child_clip.w >= 0.0 && child_clip.h >= 0.0);
    assert!(child_clip.x >= frame.x && child_clip.y >= frame.y);
    assert!(child_clip.x + child_clip.w <= frame.x + frame.w);
    assert!(child_clip.y + child_clip.h <= frame.y + frame.h);
    assert_eq!(
        child_clip.h, 0.0,
        "header and footer exhaust this compact dialog"
    );
    assert!(
        tree.visible_rect_for(child_id).is_none(),
        "a child must not leak outside an exhausted Modal body"
    );

    let invalid = Modal::new("invalid").size(f32::NAN, f32::NEG_INFINITY);
    assert!(matches!(
        invalid.snapshot_fields(),
        SnapshotFields::Modal {
            width: 0.0,
            height: 0.0,
            ..
        }
    ));
    let invalid_display = render_modal(
        &invalid.visible(true),
        Rect::new(f32::NAN, f32::INFINITY, -20.0, f32::NAN),
        (40, 40),
    );
    assert!(
        !invalid_display.contains("NaN") && !invalid_display.contains("inf"),
        "{invalid_display}"
    );
}

#[test]
fn modal_accessibility_switches_between_trigger_and_dialog_with_title_fallback() {
    let closed = Modal::new("").snapshot_fields().accessibility();
    assert_eq!(closed.role, AccessibilityRole::Button);
    assert_eq!(closed.name.as_deref(), Some("打开 Modal"));
    assert_eq!(closed.state.expanded, Some(false));

    let open = Modal::new("")
        .visible(true)
        .snapshot_fields()
        .accessibility();
    assert_eq!(open.role, AccessibilityRole::Dialog);
    assert_eq!(open.name.as_deref(), Some("Modal"));
    assert_eq!(open.state.expanded, Some(true));
}
