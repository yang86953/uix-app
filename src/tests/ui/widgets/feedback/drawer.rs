use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::pipeline::{FrameRenderInput, FrameRenderer};
use crate::draw::spatial::Orientation;
use crate::draw::traits::GraphicsEngine;
use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::feedback::drawer::*;
use crate::ui::widgets::other::scroll_view::ScrollView;

#[test]
fn closed_drawer_trigger_opens_via_widget_tree_pointer_down() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Drawer::new("Drawer")));
    tree.get_mut(id)
        .expect("drawer root")
        .set_frame(Rect::new(0.0, 0.0, 96.0, 32.0));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
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
    let drawer = tree.add_child(scroll, Box::new(Drawer::new("Drawer")));
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
fn drawer_enter_transition_advances_and_marks_paint_dirty() {
    let mut drawer = Drawer::new("Drawer").show();
    let initial_offset = drawer.transition.offset;

    assert!(WidgetAnimation::update_animation(&mut drawer, 0.05));
    assert_ne!(drawer.transition.offset, initial_offset);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Drawer::new("Drawer").show()));
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

    assert!(!WidgetAnimation::update_animation(&mut drawer, 1.0));
    assert!(!drawer.is_present());
    assert!(drawer.transition_dirty);
}
