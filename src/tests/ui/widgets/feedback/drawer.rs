use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::pipeline::{FrameRenderInput, FrameRenderer};
use crate::draw::spatial::Orientation;
use crate::draw::traits::GraphicsEngine;
use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::feedback::drawer::*;
use crate::ui::widgets::other::scroll_view::ScrollView;
use crate::ui::AnimationConfig;

fn render_drawer(drawer: &Drawer, frame: Rect, surface_size: (i32, i32)) -> String {
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
            WidgetRender::render(drawer, frame, ctx, &tree);
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
fn closed_drawer_trigger_opens_on_matching_pointer_release() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Drawer::new("Drawer")));
    tree.get_mut(id)
        .expect("drawer root")
        .set_frame(Rect::new(0.0, 0.0, 96.0, 32.0));
    assert_eq!(tree.collect_focusable(), vec![id]);

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(48.0, 16.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!tree
        .get(id)
        .expect("drawer root")
        .component()
        .as_any()
        .downcast_ref::<Drawer>()
        .expect("drawer component")
        .is_present());
    tree.get_mut(id)
        .expect("drawer root")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Drawer>()
        .expect("drawer component")
        .sync_from(Drawer::new("Drawer"));
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
        .expect("drawer root")
        .component()
        .as_any()
        .downcast_ref::<Drawer>()
        .expect("drawer component")
        .is_present());
    assert!(
        tree.collect_focusable().is_empty(),
        "an open Drawer owner must leave the Tab order reserved for its focus-trapped content"
    );
}

#[test]
fn stretched_closed_drawer_centers_its_visible_trigger_and_hit_target() {
    let mut drawer = Drawer::new("Drawer");
    let frame = Rect::new(0.0, 0.0, 300.0, 32.0);
    let commands = render_drawer(&drawer, frame, (320, 64));
    assert!(commands.contains("x: 102.0"));
    assert_eq!(
        EventHandler::hit_test_frame(&drawer, frame),
        Rect::new(102.0, 0.0, 96.0, 32.0)
    );
    assert_eq!(
        drawer.on_event(&pointer("down", Point::new(150.0, 16.0))),
        EventResult::Handled
    );
    assert_eq!(
        drawer.on_event(&pointer("up", Point::new(150.0, 16.0))),
        EventResult::Handled
    );
    assert!(drawer.is_present());
}

#[test]
fn closed_drawer_trigger_is_keyboard_accessible_on_matching_release() {
    let mut drawer = Drawer::new("Drawer");
    assert_eq!(WidgetComponent::tab_index(&drawer), 1);
    let closed_accessibility = drawer.snapshot_fields().accessibility();
    assert_eq!(
        closed_accessibility.role,
        crate::ui::AccessibilityRole::Button
    );
    assert_eq!(closed_accessibility.name.as_deref(), Some("打开 Drawer"));
    assert_eq!(closed_accessibility.state.expanded, Some(false));
    assert_eq!(
        drawer.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!drawer.is_present());
    assert_eq!(
        drawer.on_event(&SystemEvent::KeyUp {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(drawer.is_present());
    assert_eq!(WidgetComponent::tab_index(&drawer), 0);
    let open_accessibility = drawer.snapshot_fields().accessibility();
    assert_eq!(
        open_accessibility.role,
        crate::ui::AccessibilityRole::Dialog
    );
    assert_eq!(open_accessibility.name.as_deref(), Some("Drawer"));
    assert_eq!(open_accessibility.state.expanded, Some(true));

    let mut untitled = Drawer::new("");
    assert_eq!(
        untitled.snapshot_fields().accessibility().name.as_deref(),
        Some("打开 Drawer")
    );
    untitled.open();
    assert_eq!(
        untitled.snapshot_fields().accessibility().name.as_deref(),
        Some("Drawer")
    );
}

#[test]
fn masked_drawer_uses_surface_height_instead_of_its_trigger_slot() {
    let mut drawer = Drawer::new("").show();
    assert!(!WidgetAnimation::update_animation(&mut drawer, 1.0));

    let mut canvas = SharedRasterizer::new(PixelSurface::new(800, 600));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
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
            800,
            600,
        );
        WidgetRender::render(&drawer, Rect::new(120.0, 80.0, 96.0, 32.0), &mut ctx, &tree);
    }

    let panel_pixel = canvas.surface().pixels()[300 * 800 + 500];
    assert_ne!(
        panel_pixel & 0x00FF_FFFF,
        0,
        "masked drawer panel must occupy the surface, not only its trigger slot"
    );
}

#[test]
fn masked_drawer_animation_marks_its_surface_dirty_through_layer_tree() {
    let mut tree = WidgetTree::new();
    let scroll = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).size(800.0, 600.0),
    ));
    let drawer = tree.add_child(
        scroll,
        Box::new(
            Drawer::new("Drawer")
                .enter_animation(AnimationConfig::slide_in(crate::ui::Placement::Right, 0.25)),
        ),
    );
    tree.get_mut(scroll)
        .expect("scroll root")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.get_mut(drawer)
        .expect("drawer child")
        .set_frame(Rect::new(120.0, 80.0, 96.0, 32.0));

    let mut engine = SoftwareEngine::new();
    engine.initialize(800, 600).expect("software engine");
    let mut renderer = FrameRenderer::new();
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let first_frame = DirtyRegion::full();
    renderer.render_frame(
        &mut engine,
        &tree,
        FrameRenderInput {
            rendered_first: false,
            dirty_region: &first_frame,
            tree_version: tree.tree_version(),
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font,
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    tree.reset_invalidation();

    tree.invalidate_paint(drawer);
    let closed_dirty = tree.dirty_region();
    renderer.render_frame(
        &mut engine,
        &tree,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &closed_dirty,
            tree_version: tree.tree_version(),
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font,
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    assert!(
        renderer
            .render_object_tree()
            .get(drawer)
            .is_some_and(|entry| entry.display_list.is_some()),
        "the closed trigger must be recorded before opening the Drawer"
    );
    tree.reset_invalidation();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(168.0, 96.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: Point::new(168.0, 96.0),
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(tree.update(0.05), "enter animation should still be active");

    let opening_dirty = tree.dirty_region();
    renderer.render_frame(
        &mut engine,
        &tree,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &opening_dirty,
            tree_version: tree.tree_version(),
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font,
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );
    tree.reset_invalidation();

    assert!(tree.update(0.05), "enter animation should continue");
    let dirty = tree.dirty_region();
    assert!(
        dirty.intersects(Rect::new(700.0, 300.0, 1.0, 1.0)),
        "an animated masked Drawer must dirty its right-side panel, not only its 96x32 trigger"
    );
    renderer.render_frame(
        &mut engine,
        &tree,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &dirty,
            tree_version: tree.tree_version(),
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font,
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    let panel_pixel = engine
        .session()
        .cpu_backend()
        .expect("cpu backend")
        .pixels()[300 * 800 + 700];
    assert_ne!(
        panel_pixel & 0x00FF_FFFF,
        0,
        "the detached Drawer layer must paint its animated right-side panel"
    );

    assert!(
        !tree.update(1.0),
        "the enter animation should have reached its final state"
    );
    tree.reset_invalidation();
    tree.invalidate_paint(scroll);
    let root_redraw = tree.dirty_region();
    renderer.render_frame(
        &mut engine,
        &tree,
        FrameRenderInput {
            rendered_first: true,
            dirty_region: &root_redraw,
            tree_version: tree.tree_version(),
            scroll_move: None,
            theme: ThemeSnapshot::new(&tokens),
            font,
            font_service: &fonts,
            image_service: &images,
            debug_mode: false,
            hover_pos: None,
            metrics: None,
        },
    );

    let settled_panel_pixel = engine
        .session()
        .cpu_backend()
        .expect("cpu backend")
        .pixels()[300 * 800 + 700];
    assert_ne!(
        settled_panel_pixel & 0x00FF_FFFF,
        0,
        "a finished masked Drawer must survive a later root redraw despite its zero layout slot"
    );
}

#[test]
fn drawer_advertises_animation_capability() {
    let drawer = Drawer::new("Drawer").show();

    assert!(drawer
        .capabilities()
        .contains(WidgetCapabilities::ANIMATION));
    assert!(drawer.as_animation().is_some());
}

#[test]
fn drawer_default_enter_transition_is_immediately_at_rest() {
    let mut drawer = Drawer::new("Drawer").show();

    assert!(drawer.transition.finished);
    assert_eq!(drawer.transition.opacity_progress, 1.0);
    assert_eq!(drawer.transition.offset, Point::new(0.0, 0.0));
    assert!(!WidgetAnimation::update_animation(&mut drawer, 0.05));
}

#[test]
fn drawer_enter_transition_advances_and_marks_paint_dirty() {
    let mut drawer = Drawer::new("Drawer")
        .enter_animation(AnimationConfig::slide_in(crate::ui::Placement::Right, 0.25))
        .show();
    let initial_offset = drawer.transition.offset;

    assert!(WidgetAnimation::update_animation(&mut drawer, 0.05));
    assert_ne!(drawer.transition.offset, initial_offset);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(
        Drawer::new("Drawer")
            .enter_animation(AnimationConfig::slide_in(crate::ui::Placement::Right, 0.25))
            .show(),
    ));
    tree.get_mut(id)
        .expect("drawer root")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.get_mut(id).expect("drawer root").set_active(true);
    tree.invalidation().lock().unwrap().clear();

    assert!(tree.update(1.0 / 60.0));

    let queue = tree.invalidation().lock().unwrap();
    assert!(queue.has_paint_or_composite());
    assert!(queue.node_needs_paint(id));
}

#[test]
fn drawer_close_finishes_exit_transition_before_internal_hide() {
    let mut drawer = Drawer::new("Drawer").show();
    assert!(!WidgetAnimation::update_animation(&mut drawer, 1.0));
    assert!(drawer.is_visible());

    drawer.close();
    assert!(!drawer.is_visible());
    assert!(drawer.is_present());
    assert!(drawer.take_layout_request());

    assert!(!WidgetAnimation::update_animation(&mut drawer, 1.0));
    assert!(!drawer.is_present());
    assert!(drawer.transition_dirty);
    assert!(
        drawer.take_layout_request(),
        "finishing the overlay leave transition must restore the closed trigger slot"
    );
}

#[test]
fn closed_drawer_hides_its_retained_child_subtree() {
    use crate::ui::view::{button, ViewAdapter, ViewNode};

    let mut tree = ViewAdapter::build(ViewNode::new(
        Drawer::new("Drawer").visible(true),
        vec![button("Cancel").into()],
    ));
    let drawer = tree.root_id().expect("drawer root");
    tree.get_mut(drawer)
        .expect("drawer node")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.get_mut(drawer).expect("drawer node").set_active(true);
    tree.layout();
    assert!(!tree.update(1.0));
    tree.layout();

    let child = tree
        .get(drawer)
        .expect("drawer node")
        .children()
        .first()
        .copied()
        .expect("drawer child");
    assert!(tree.visible_rect_for(child).is_some());

    tree.get_mut(drawer)
        .expect("drawer node")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Drawer>()
        .expect("Drawer component")
        .close();
    assert!(!tree.update(1.0));

    assert!(
        tree.visible_rect_for(child).is_none(),
        "a closed Drawer must not expose stale child geometry"
    );
    assert_ne!(
        tree.hit_test(Point::new(650.0, 100.0)),
        Some(child),
        "a closed Drawer child must not remain hittable"
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
        "reopening a Drawer must relayout and reveal its retained child subtree"
    );
}

#[test]
fn drawer_uses_custom_enter_and_leave_animations() {
    let mut drawer = Drawer::new("Drawer")
        .placement(DrawerPlacement::Left)
        .enter_animation(AnimationConfig::fade_in(0.4))
        .leave_animation(AnimationConfig::fade_out(0.3))
        .show();

    assert_eq!(drawer.transition.offset, Point::new(0.0, 0.0));
    assert!(WidgetAnimation::update_animation(&mut drawer, 0.2));

    drawer.close();
    assert!(WidgetAnimation::update_animation(&mut drawer, 0.2));
    assert!(drawer.is_present());
    assert!(!WidgetAnimation::update_animation(&mut drawer, 0.1));
    assert!(!drawer.is_present());
}

#[test]
fn drawer_close_button_requires_matching_release_and_cancels_on_leave() {
    let mut drawer = Drawer::new("Drawer").size(200.0, 180.0).show();
    assert!(!WidgetAnimation::update_animation(&mut drawer, 1.0));
    render_drawer(&drawer, Rect::zero(), (320, 240));
    // Right drawer on 320x240 surface: panel x=120, close slot x=272..320.
    let close = Point::new(296.0, 24.0);

    assert_eq!(
        drawer.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert!(drawer.is_visible(), "PointerDown must not close Drawer");
    assert_eq!(
        drawer.on_event(&pointer("up", Point::new(20.0, 80.0))),
        EventResult::Handled
    );
    assert!(drawer.is_visible(), "release outside must cancel close");

    assert_eq!(
        drawer.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(
        drawer.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(drawer.on_event(&pointer("up", close)), EventResult::Handled);
    assert!(drawer.is_visible(), "PointerLeave must cancel close");

    assert_eq!(
        drawer.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    assert_eq!(drawer.on_event(&pointer("up", close)), EventResult::Handled);
    assert!(!drawer.is_visible());
    assert!(drawer.is_present(), "leave transition must remain present");
}

#[test]
fn masked_drawer_closes_via_escape_close_button_and_mask_in_widget_tree() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        ScrollView::new(ScrollDirection::Vertical).size(800.0, 600.0),
    ));
    let drawer_id = tree.add_child(root, Box::new(Drawer::new("Drawer")));
    tree.get_mut(root)
        .expect("scroll root")
        .set_frame(Rect::new(0.0, 0.0, 800.0, 600.0));
    tree.get_mut(drawer_id)
        .expect("drawer child")
        .set_frame(Rect::new(120.0, 80.0, 96.0, 32.0));

    fn trigger_point(tree: &WidgetTree, drawer_id: crate::ui::ComponentId) -> Point {
        let frame = tree.get(drawer_id).expect("drawer child").frame();
        Point::new(frame.x + frame.w * 0.5, frame.y + frame.h * 0.5)
    }

    let trigger = trigger_point(&tree, drawer_id);

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: trigger,
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: trigger,
            button: crate::ui::MouseButton::Left,
            mods: crate::native::traits::input::KeyMod::NONE,
        }),
        EventResult::Handled
    );
    tree.layout();
    assert!(!tree.update(1.0), "default enter must finish immediately");

    fn drawer_is_visible(tree: &WidgetTree, drawer_id: crate::ui::ComponentId) -> bool {
        tree.get(drawer_id)
            .expect("drawer child")
            .component()
            .as_any()
            .downcast_ref::<Drawer>()
            .expect("drawer component")
            .is_visible()
    }

    fn drawer_is_present(tree: &WidgetTree, drawer_id: crate::ui::ComponentId) -> bool {
        tree.get(drawer_id)
            .expect("drawer child")
            .component()
            .as_any()
            .downcast_ref::<Drawer>()
            .expect("drawer component")
            .is_present()
    }

    assert!(drawer_is_visible(&tree, drawer_id));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Escape,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!drawer_is_visible(&tree, drawer_id));
    assert!(drawer_is_present(&tree, drawer_id));
    assert!(!tree.update(1.0));
    assert!(!drawer_is_present(&tree, drawer_id));
    tree.layout();

    let trigger = trigger_point(&tree, drawer_id);

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: trigger,
            button: crate::ui::MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: trigger,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    tree.layout();
    assert!(drawer_is_visible(&tree, drawer_id));

    // Right drawer width 378 on 800 surface → panel x=422, close slot center ≈ (770, 24).
    assert_eq!(
        tree.dispatch_event(&pointer("down", Point::new(770.0, 24.0))),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&pointer("up", Point::new(770.0, 24.0))),
        EventResult::Handled
    );
    assert!(!drawer_is_visible(&tree, drawer_id));
    assert!(!tree.update(1.0));
    assert!(!drawer_is_present(&tree, drawer_id));
    tree.layout();

    let trigger = trigger_point(&tree, drawer_id);

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: trigger,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos: trigger,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    tree.layout();
    assert!(drawer_is_visible(&tree, drawer_id));

    assert_eq!(
        tree.dispatch_event(&pointer("down", Point::new(200.0, 300.0))),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&pointer("up", Point::new(200.0, 300.0))),
        EventResult::Handled
    );
    assert!(!drawer_is_visible(&tree, drawer_id));
    assert!(!tree.update(1.0));
    assert!(!drawer_is_present(&tree, drawer_id));
}

#[test]
fn drawer_mask_reconcile_cancels_an_armed_close() {
    let mut drawer = Drawer::new("Drawer").size(200.0, 180.0).show();
    assert!(!WidgetAnimation::update_animation(&mut drawer, 1.0));
    render_drawer(&drawer, Rect::zero(), (320, 240));
    let close = Point::new(296.0, 24.0);

    assert_eq!(
        drawer.on_event(&pointer("down", close)),
        EventResult::Handled
    );
    drawer.sync_from(Drawer::new("Drawer").size(200.0, 180.0).mask(false));
    assert_eq!(drawer.on_event(&pointer("up", close)), EventResult::Handled);
    assert!(
        drawer.is_visible(),
        "changing overlay coordinates must cancel the armed close"
    );
}

#[test]
fn oversized_drawer_clamps_to_surface_and_clips_header_footer_and_children() {
    let mut drawer = Drawer::new("超长 Drawer 标题 mixed title")
        .size(500.0, 500.0)
        .extra("超长 extra action")
        .footer_visible(true)
        .show();
    assert!(!WidgetAnimation::update_animation(&mut drawer, 1.0));
    let display_list = render_drawer(&drawer, Rect::zero(), (132, 90));

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 0.0, y: 0.0, w: 132.0, h: 90.0 } }"),
        "Drawer panel must clamp and clip to the real surface: {display_list}"
    );
    assert!(
        display_list.contains('…'),
        "long Drawer header text must elide: {display_list}"
    );
    assert!(!display_list.contains("w: -") && !display_list.contains("h: -"));
    assert_eq!(
        WidgetRender::children_clip(&drawer, Rect::zero()),
        Some(Rect::new(0.0, 48.0, 132.0, 0.0)),
        "header and footer exhaust this compact panel without negative body geometry"
    );

    let invalid = Drawer::new("invalid").size(f32::NAN, f32::NEG_INFINITY);
    assert!(matches!(
        invalid.snapshot_fields(),
        SnapshotFields::Drawer {
            width: 0.0,
            height: 0.0,
            ..
        }
    ));
}
