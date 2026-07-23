use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::draw::Canvas2D;
use crate::tests::common::*;
use crate::ui::widgets::ImageGroup;
use crate::ui::AccessibilityRole;

const DEMO_IMAGE: &str = "assets/images/demo.png";

fn render(group: &ImageGroup, canvas: &mut SharedRasterizer, frame: Rect) {
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let width = canvas.width();
    let height = canvas.height();
    let mut ctx = PaintContext::new_for_test(
        canvas,
        font,
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        width,
        height,
    );
    WidgetRender::render(group, frame, &mut ctx, &tree);
}

fn render_display_list(group: &ImageGroup, frame: Rect, surface_size: (i32, i32)) -> String {
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
            WidgetRender::render(group, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn left_key() -> SystemEvent {
    SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    }
}

fn right_key() -> SystemEvent {
    SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    }
}

fn pointer_down(pos: Point) -> SystemEvent {
    SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

#[test]
fn gallery_paints_a_real_main_image_and_thumbnail_list() {
    let group = ImageGroup::new().images([DEMO_IMAGE, DEMO_IMAGE, DEMO_IMAGE]);
    let display_list = render_display_list(&group, Rect::new(10.0, 8.0, 320.0, 220.0), (360, 250));

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 8.0, w: 320.0, h: 220.0 } }"),
        "gallery paint must stay clipped to its frame: {display_list}"
    );
    assert!(
        display_list.matches("DrawImage").count() >= 4,
        "the main image and all three visible thumbnails must use decoded image paint: {display_list}"
    );
}

#[test]
fn arrows_and_keyboard_navigation_wrap_and_emit_only_real_changes() {
    let mut group = ImageGroup::new()
        .images(["a.png", "b.png", "c.png"])
        .start_index(1);
    let right = right_key();

    assert_eq!(group.on_event(&right), EventResult::Handled);
    assert_eq!(group.current_index(), 2);
    assert_eq!(
        group
            .semantic_event(ComponentId::new(17), &right)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("2".to_owned())
    );
    assert_eq!(group.on_event(&right), EventResult::Handled);
    assert_eq!(group.current_index(), 0);
    assert_eq!(group.on_event(&left_key()), EventResult::Handled);
    assert_eq!(group.current_index(), 2);
}

#[test]
fn snapshot_and_accessibility_expose_the_live_gallery_and_dialog_state() {
    let mut group = ImageGroup::new()
        .images(["a.png", "b.png", "c.png"])
        .start_index(1);
    assert_eq!(
        group.snapshot_fields(),
        SnapshotFields::ImageGroup {
            images: vec!["a.png".into(), "b.png".into(), "c.png".into()],
            start_index: 1,
            current: 1,
            preview_open: false,
        }
    );
    let closed = group.snapshot_fields().accessibility();
    assert_eq!(closed.role, AccessibilityRole::Button);
    assert_eq!(closed.name.as_deref(), Some("图片组"));
    assert_eq!(closed.state.expanded, Some(false));
    assert_eq!(closed.state.value_now, Some(2.0));
    assert_eq!(closed.state.value_max, Some(3.0));
    assert_eq!(
        closed.state.value_text.as_deref(),
        Some("b.png，图片 2 / 3")
    );

    group.open_preview();
    let open = group.snapshot_fields().accessibility();
    assert_eq!(open.role, AccessibilityRole::Dialog);
    assert_eq!(open.name.as_deref(), Some("图片预览"));
    assert_eq!(open.state.expanded, Some(true));
}

#[test]
fn inline_pointer_hit_zones_match_arrows_thumbnails_and_main_preview() {
    let mut group = ImageGroup::new().images([DEMO_IMAGE, DEMO_IMAGE, DEMO_IMAGE]);
    let mut canvas = SharedRasterizer::new(PixelSurface::new(640, 480));
    render(&group, &mut canvas, Rect::new(10.0, 20.0, 320.0, 220.0));

    // Normal geometry: thumbnails are 44px squares at x=88/138/188 and y=170.
    assert_eq!(
        group.on_event(&pointer_down(Point::new(210.0, 190.0))),
        EventResult::Handled
    );
    assert_eq!(group.current_index(), 2);
    assert_eq!(
        group.on_event(&pointer_down(Point::new(10.0, 80.0))),
        EventResult::Handled
    );
    assert_eq!(group.current_index(), 1);

    assert_eq!(
        group.on_event(&pointer_down(Point::new(160.0, 80.0))),
        EventResult::Handled
    );
    assert!(group.is_preview_open());
}

#[test]
fn preview_is_a_full_surface_modal_with_real_navigation_and_mask_close() {
    let frame = Rect::new(40.0, 30.0, 320.0, 220.0);
    let group = ImageGroup::new().images([DEMO_IMAGE, DEMO_IMAGE, DEMO_IMAGE]);
    let mut canvas = SharedRasterizer::new(PixelSurface::new(640, 480));
    render(&group, &mut canvas, frame);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(group));
    tree.get_mut(id).expect("gallery root").set_frame(frame);

    assert_eq!(
        tree.dispatch_event(&pointer_down(Point::new(200.0, 100.0))),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(id));
    let overlay = tree.overlay_stack().top().expect("gallery preview overlay");
    assert_eq!(overlay.owner(), id);
    assert_eq!(overlay.kind(), OverlayKind::Modal);
    assert!(overlay.is_modal());
    assert!(overlay.traps_focus());
    assert_eq!(
        overlay.bounds_rect(),
        Some(Rect::new(0.0, 0.0, 640.0, 480.0))
    );

    let group = tree
        .get(id)
        .and_then(|node| node.component().as_any().downcast_ref::<ImageGroup>())
        .expect("gallery component");
    assert_eq!(
        EventHandler::hit_test_frame(group, frame),
        Rect::new(0.0, 0.0, 640.0, 480.0)
    );
    assert_eq!(
        WidgetRender::dirty_rect(group, frame),
        Rect::new(0.0, 0.0, 640.0, 480.0)
    );

    assert_eq!(tree.dispatch_event(&right_key()), EventResult::Handled);
    assert_eq!(
        tree.get(id)
            .and_then(|node| node.component().as_any().downcast_ref::<ImageGroup>())
            .map(ImageGroup::current_index),
        Some(1)
    );
    assert_eq!(
        tree.dispatch_event(&pointer_down(Point::new(320.0, 200.0))),
        EventResult::Handled,
        "clicking the preview image is consumed but does not dismiss it"
    );
    assert!(tree.overlay_stack().top().is_some());

    assert_eq!(
        tree.dispatch_event(&pointer_down(Point::new(4.0, 4.0))),
        EventResult::Handled
    );
    assert!(tree.overlay_stack().is_empty());
    assert!(tree
        .get(id)
        .and_then(|node| node.component().as_any().downcast_ref::<ImageGroup>())
        .is_some_and(|group| !group.is_preview_open()));
}

#[test]
fn preview_paints_the_selected_image_and_clears_the_surface_after_escape() {
    let frame = Rect::new(40.0, 30.0, 320.0, 220.0);
    let mut group = ImageGroup::new().images([DEMO_IMAGE, DEMO_IMAGE, DEMO_IMAGE]);
    let mut canvas = SharedRasterizer::new(PixelSurface::new(640, 480));
    render(&group, &mut canvas, frame);
    group.open_preview();

    let display_list = render_display_list(&group, frame, (640, 480));
    assert!(
        display_list.matches("DrawImage").count() >= 8,
        "inline and modal gallery paths must both paint decoded images: {display_list}"
    );
    assert!(display_list.contains("1 / 3"), "{display_list}");
    assert_eq!(
        WidgetRender::dirty_rect(&group, frame),
        Rect::new(0.0, 0.0, 640.0, 480.0)
    );
    assert_eq!(
        group.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Escape,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!group.is_preview_open());
    assert_eq!(
        WidgetRender::dirty_rect(&group, frame),
        Rect::new(0.0, 0.0, 640.0, 480.0),
        "the first closed frame must erase the old modal pixels"
    );
    render(&group, &mut canvas, frame);
    assert_eq!(WidgetRender::dirty_rect(&group, frame), frame);
}

#[test]
fn empty_and_single_image_groups_do_not_fake_navigation() {
    let mut empty = ImageGroup::new();
    assert_eq!(empty.tab_index(), 0);
    assert_eq!(empty.on_event(&right_key()), EventResult::NotHandled);
    empty.open_preview();
    assert!(!empty.is_preview_open());
    assert!(WidgetRender::overlay_entry(
        &empty,
        ComponentId::new(1),
        Rect::new(0.0, 0.0, 320.0, 220.0)
    )
    .is_none());

    let mut single = ImageGroup::new().images([DEMO_IMAGE]);
    assert_eq!(single.tab_index(), 1);
    assert_eq!(single.on_event(&right_key()), EventResult::NotHandled);
    assert_eq!(single.current_index(), 0);
    assert!(single
        .semantic_event(ComponentId::new(2), &right_key())
        .is_none());
    assert_eq!(
        single.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(single.is_preview_open());
}

#[test]
fn reconcile_preserves_selected_image_and_preview_until_the_group_becomes_empty() {
    let mut group = ImageGroup::new().images(["a.png", "b.png", "c.png"]);
    assert_eq!(group.on_event(&right_key()), EventResult::Handled);
    group.open_preview();

    group.sync_from(ImageGroup::new().images(["c.png", "a.png", "b.png"]));
    assert_eq!(
        group.current_index(),
        2,
        "the selected path moved to index 2"
    );
    assert!(group.is_preview_open());

    group.sync_from(ImageGroup::new().images(["only.png"]));
    assert_eq!(group.current_index(), 0);
    assert!(group.is_preview_open());

    group.sync_from(ImageGroup::new());
    assert_eq!(group.current_index(), 0);
    assert!(!group.is_preview_open());
    assert_eq!(group.tab_index(), 0);
}

#[test]
fn reconcile_preserves_the_selected_occurrence_when_paths_repeat() {
    let mut group = ImageGroup::new().images([DEMO_IMAGE, DEMO_IMAGE, DEMO_IMAGE]);
    assert_eq!(group.on_event(&left_key()), EventResult::Handled);
    assert_eq!(group.current_index(), 2);

    group.sync_from(ImageGroup::new().images([DEMO_IMAGE, DEMO_IMAGE, DEMO_IMAGE]));
    assert_eq!(group.current_index(), 2);
}

#[test]
fn authored_config_comparison_ignores_live_index_and_preview_state() {
    let mut current = ImageGroup::new().images(["a.png", "b.png", "c.png"]);
    assert_eq!(current.on_event(&right_key()), EventResult::Handled);
    current.open_preview();
    let same_config = ImageGroup::new().images(["a.png", "b.png", "c.png"]);
    let changed_config = ImageGroup::new().images(["a.png", "b.png"]);

    assert_eq!(
        crate::ui::component_patch::builtin_widget_config_changed(&current, &same_config),
        Some(false)
    );
    assert_eq!(
        crate::ui::component_patch::builtin_widget_config_changed(&current, &changed_config),
        Some(true)
    );
}

#[test]
fn non_finite_and_tiny_frames_cannot_escape_or_poison_paint() {
    let group = ImageGroup::new().images([DEMO_IMAGE, DEMO_IMAGE]);
    let invalid = render_display_list(
        &group,
        Rect::new(f32::NAN, f32::INFINITY, -20.0, f32::NAN),
        (40, 40),
    );
    assert!(
        !invalid.contains("NaN") && !invalid.contains("inf"),
        "{invalid}"
    );

    let tiny = render_display_list(&group, Rect::new(3.0, 4.0, 12.0, 8.0), (24, 20));
    assert!(!tiny.contains("w: -") && !tiny.contains("h: -"), "{tiny}");
    assert!(
        tiny.contains("PushClip { rect: Rect { x: 3.0, y: 4.0, w: 12.0, h: 8.0 } }"),
        "{tiny}"
    );
}
