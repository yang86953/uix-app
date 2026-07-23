use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::draw::renderer::scene_pipeline::{FrameRenderInput, ScenePipeline};
use crate::draw::Canvas2D;
use crate::tests::common::*;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::{Button, Image, Label};
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

fn render_display_list(image: &Image, frame: Rect, surface_size: (i32, i32)) -> String {
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
            WidgetRender::render(image, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn render_with_image_service(
    image: &Image,
    images: &ImageService,
    frame: Rect,
    surface_size: (i32, i32),
) {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        font,
        &fonts,
        images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        surface_size.0,
        surface_size.1,
    );
    WidgetRender::render(image, frame, &mut ctx, &tree);
}

struct ImageTreeHarness {
    engine: Renderer,
    renderer: ScenePipeline,
    fonts: FontService,
    font: crate::draw::FontHandle,
    images: ImageService,
    tokens: DesignTokens,
    rendered_first: bool,
}

impl ImageTreeHarness {
    fn new() -> Self {
        let mut engine = Renderer::cpu();
        engine.initialize(160, 120).expect("CPU renderer");
        let mut fonts = FontService::new();
        let font = fonts
            .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
            .expect("load deterministic test font");
        Self {
            engine,
            renderer: ScenePipeline::new(),
            fonts,
            font,
            images: ImageService::new(),
            tokens: DesignTokens::antd_light(),
            rendered_first: false,
        }
    }

    fn render(&mut self, tree: &WidgetTree) {
        let dirty = DirtyRegion::full();
        let _ = self.renderer.render_frame(
            &mut self.engine,
            tree,
            FrameRenderInput {
                rendered_first: self.rendered_first,
                dirty_region: &dirty,
                tree_version: tree.tree_version(),
                scroll_move: None,
                theme: ThemeSnapshot::new(&self.tokens),
                font: self.font,
                font_service: &self.fonts,
                image_service: &self.images,
                debug_mode: false,
                hover_pos: None,
                metrics: None,
            },
        );
        self.rendered_first = true;
    }
}

#[test]
fn loaded_image_clips_thumbnail_paint_and_keeps_alt_semantic_only() {
    let image = Image::new(128.0, 88.0)
        .src("assets/images/demo.png")
        .alt("UIX demo");
    let display_list = render_display_list(&image, Rect::new(10.0, 8.0, 72.0, 40.0), (120, 80));

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 8.0, w: 72.0, h: 40.0 } }"),
        "thumbnail paint must be clipped to the actual frame: {display_list}"
    );
    assert!(display_list.contains("DrawImage"), "{display_list}");
    assert!(
        !display_list.contains("UIX demo"),
        "alt text must not be painted below the image: {display_list}"
    );
    assert!(
        !display_list.contains('🔍'),
        "preview affordance must use the shared Lucide icon: {display_list}"
    );
}

#[test]
fn load_failure_wraps_readable_fallback_inside_constrained_frame() {
    let image = Image::new(128.0, 88.0)
        .src("assets/images/missing.png")
        .fallback("图片加载失败，请检查网络后重试");
    let display_list = render_display_list(&image, Rect::new(4.0, 3.0, 72.0, 40.0), (100, 64));

    assert!(display_list.contains("DrawTextWrapped"), "{display_list}");
    assert!(
        !display_list.contains("w: -") && !display_list.contains("h: -"),
        "{display_list}"
    );
}

#[test]
fn invalid_dimensions_and_radius_cannot_poison_layout_or_paint() {
    let image = Image::new(f32::NAN, -40.0).radius(f32::INFINITY);
    assert_eq!(
        image.measure(Constraints::loose(Size::new(200.0, 200.0))),
        Size::new(0.0, 0.0)
    );

    let display_list = render_display_list(&image, Rect::new(0.0, 0.0, f32::NAN, -10.0), (40, 40));
    assert!(!display_list.contains("NaN"), "{display_list}");
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

#[test]
fn lazy_image_defers_io_until_it_enters_the_visible_frame() {
    let images = ImageService::new();
    let image = Image::new(80.0, 48.0)
        .src("assets/images/demo.png")
        .lazy(true);

    render_with_image_service(
        &image,
        &images,
        Rect::new(240.0, 0.0, 80.0, 48.0),
        (120, 80),
    );
    assert_eq!(
        images.memory_usage(),
        0,
        "an offscreen lazy Image must not read or decode its source"
    );

    render_with_image_service(&image, &images, Rect::new(8.0, 8.0, 80.0, 48.0), (120, 80));
    assert!(
        images.memory_usage() > 0,
        "a lazy Image without a placeholder loads on its first visible opportunity"
    );
}

#[test]
fn placeholder_is_presented_before_loading_and_hidden_after_successful_layout_sync() {
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Image::new(80.0, 48.0)
            .src("assets/images/demo.png")
            .placeholder(crate::ui::view::label("加载中"))
            .preview(false),
    ));
    tree.layout();
    let placeholder = tree.find_by_type::<Label>().expect("placeholder child");
    assert!(tree.is_effectively_visible(placeholder));

    let mut harness = ImageTreeHarness::new();
    harness.render(&tree);
    assert_eq!(
        harness.images.memory_usage(),
        0,
        "the first rendered frame must not synchronously decode the image"
    );
    assert!(
        tree.is_effectively_visible(placeholder),
        "the placeholder must be part of the first presented tree"
    );

    tree.layout();
    harness.render(&tree);
    assert!(harness.images.memory_usage() > 0);
    assert!(
        tree.is_effectively_visible(placeholder),
        "the already presented placeholder remains until the load result is laid out"
    );

    tree.layout();
    assert!(
        !tree.is_effectively_visible(placeholder),
        "successful loading must remove the placeholder from paint and hit testing"
    );
}

#[test]
fn real_load_failure_materializes_specific_interactive_error_view() {
    let errors = Rc::new(RefCell::new(Vec::<String>::new()));
    let clicks = Rc::new(Cell::new(0usize));
    let errors_for_factory = Rc::clone(&errors);
    let clicks_for_factory = Rc::clone(&clicks);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Image::new(80.0, 48.0)
            .src("assets/images/definitely-missing-image.png")
            .placeholder(crate::ui::view::label("加载中"))
            .on_error(move |error| {
                errors_for_factory.borrow_mut().push(error.to_owned());
                let clicks = Rc::clone(&clicks_for_factory);
                crate::ui::view::button("重试").on_click_fn(move || {
                    clicks.set(clicks.get() + 1);
                })
            })
            .preview(false),
    ));
    tree.layout();
    let placeholder = tree.find_by_type::<Label>().expect("placeholder child");
    let mut harness = ImageTreeHarness::new();

    harness.render(&tree);
    assert!(errors.borrow().is_empty());
    tree.layout();
    harness.render(&tree);
    assert!(errors.borrow().is_empty());

    tree.layout();
    let retry = tree.find_by_type::<Button>().expect("runtime error action");
    assert!(!tree.is_effectively_visible(placeholder));
    assert!(tree.is_effectively_visible(retry));
    let errors = errors.borrow();
    assert_eq!(
        errors.len(),
        1,
        "the error factory must run once per failure"
    );
    assert!(
        errors[0].contains("definitely-missing-image.png") && errors[0].contains("读取图片文件"),
        "factory must receive the concrete image-service error: {}",
        errors[0]
    );
    drop(errors);

    let click = |down| {
        if down {
            SystemEvent::PointerDown {
                pos: Point::new(40.0, 24.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            }
        } else {
            SystemEvent::PointerUp {
                pos: Point::new(40.0, 24.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            }
        }
    };
    assert_eq!(tree.dispatch_event(&click(true)), EventResult::Handled);
    assert_eq!(tree.dispatch_event(&click(false)), EventResult::Handled);
    assert_eq!(
        clicks.get(),
        1,
        "the custom error Button must be interactive"
    );
}

#[test]
fn reconcile_to_a_new_source_clears_error_and_reuses_the_placeholder_until_ready() {
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Image::new(80.0, 48.0)
            .src("assets/images/definitely-missing-image.png")
            .placeholder(crate::ui::view::label("加载中"))
            .on_error(|_| crate::ui::view::button("重试"))
            .preview(false),
    ));
    tree.layout();
    let mut harness = ImageTreeHarness::new();
    harness.render(&tree);
    tree.layout();
    harness.render(&tree);
    tree.layout();
    assert!(tree.find_by_type::<Button>().is_some());

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Image::new(80.0, 48.0)
                .src("assets/images/demo.png")
                .placeholder(crate::ui::view::label("加载中"))
                .on_error(|_| crate::ui::view::button("重试"))
                .preview(false),
        ),
    );
    tree.layout();
    assert!(
        tree.find_by_type::<Button>().is_none(),
        "changing src must remove the stale recovery subtree"
    );
    let placeholder = tree.find_by_type::<Label>().expect("new placeholder");
    assert!(tree.is_effectively_visible(placeholder));

    harness.render(&tree);
    tree.layout();
    harness.render(&tree);
    tree.layout();
    assert!(harness.images.memory_usage() > 0);
    assert!(!tree.is_effectively_visible(placeholder));
}
