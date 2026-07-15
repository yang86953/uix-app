use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::draw::traits::canvas::Canvas2D;
use crate::tests::common::*;
use crate::ui::widgets::Image;
use crate::ui::{AccessibilityRole, ComponentId, EventHandler, WidgetRender};

fn render(image: &Image, canvas: &mut SharedRasterizer, frame: Rect) {
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
    WidgetRender::render(image, frame, &mut ctx, &tree);
}

#[test]
fn preview_uses_full_surface_overlay_and_clears_it_after_close() {
    let frame = Rect::new(40.0, 30.0, 120.0, 80.0);
    let mut image = Image::new(120.0, 80.0)
        .src("assets/images/demo.png")
        .alt("产品图");
    let mut canvas = SharedRasterizer::new(PixelSurface::new(640, 480));
    render(&image, &mut canvas, frame);

    assert_eq!(image.tab_index(), 1);
    assert_eq!(
        image.on_event(&SystemEvent::PointerDown {
            pos: Point::new(20.0, 20.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(image.is_preview_open());
    assert_eq!(
        EventHandler::hit_test_frame(&image, frame),
        Rect::new(0.0, 0.0, 640.0, 480.0)
    );
    let overlay = WidgetRender::overlay_entry(&image, ComponentId::new(7), frame)
        .expect("open preview overlay");
    assert_eq!(overlay.kind(), crate::ui::OverlayKind::Modal);
    assert!(overlay.is_modal());
    assert!(overlay.traps_focus());
    assert_eq!(
        overlay.bounds_rect(),
        Some(Rect::new(0.0, 0.0, 640.0, 480.0))
    );

    render(&image, &mut canvas, frame);
    assert_eq!(
        WidgetRender::dirty_rect(&image, frame),
        Rect::new(0.0, 0.0, 640.0, 480.0)
    );
    assert_eq!(
        image.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Escape,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!image.is_preview_open());
    assert_eq!(
        WidgetRender::dirty_rect(&image, frame),
        Rect::new(0.0, 0.0, 640.0, 480.0)
    );
    render(&image, &mut canvas, frame);
    assert_eq!(WidgetRender::dirty_rect(&image, frame), frame);
}

#[test]
fn keyboard_preview_state_is_accessible_and_disabled_preview_is_static() {
    let mut image = Image::new(80.0, 60.0).alt("头像");
    assert_eq!(
        image.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let accessibility = image.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.name.as_deref(), Some("头像"));
    assert_eq!(accessibility.state.expanded, Some(true));

    let mut static_image = Image::new(80.0, 60.0).alt("头像").preview(false);
    assert_eq!(static_image.tab_index(), 0);
    assert_eq!(
        static_image.on_event(&SystemEvent::PointerDown {
            pos: Point::new(1.0, 1.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    static_image.open_preview();
    assert!(!static_image.is_preview_open());
    let accessibility = static_image.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Image);
    assert_eq!(accessibility.state.expanded, None);
}

#[test]
fn widget_tree_routes_full_surface_clicks_to_the_open_preview() {
    let frame = Rect::new(40.0, 30.0, 120.0, 80.0);
    let image = Image::new(120.0, 80.0).alt("产品图");
    let mut canvas = SharedRasterizer::new(PixelSurface::new(640, 480));
    render(&image, &mut canvas, frame);

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(image));
    tree.get_mut(id).expect("image root").set_frame(frame);
    let pointer_down = |pos| SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(
        tree.dispatch_event(&pointer_down(Point::new(80.0, 60.0))),
        EventResult::Handled
    );
    assert!(tree
        .overlay_stack()
        .top()
        .is_some_and(|entry| entry.owner() == id));
    assert!(tree
        .get(id)
        .and_then(|node| node.component().as_any().downcast_ref::<Image>())
        .is_some_and(Image::is_preview_open));

    assert_eq!(
        tree.dispatch_event(&pointer_down(Point::new(600.0, 440.0))),
        EventResult::Handled
    );
    assert!(tree.overlay_stack().is_empty());
    assert!(tree
        .get(id)
        .and_then(|node| node.component().as_any().downcast_ref::<Image>())
        .is_some_and(|image| !image.is_preview_open()));
}

#[test]
fn reconcile_preserves_open_preview_until_preview_is_disabled() {
    let mut image = Image::new(120.0, 80.0).src("before.png");
    image.open_preview();
    image.sync_from(Image::new(160.0, 100.0).src("after.png"));
    assert!(image.is_preview_open());
    assert_eq!(
        image.snapshot_fields().accessibility().state.expanded,
        Some(true)
    );

    image.sync_from(Image::new(160.0, 100.0).preview(false));
    assert!(!image.is_preview_open());
}
