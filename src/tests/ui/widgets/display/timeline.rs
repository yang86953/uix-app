use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::draw::traits::canvas::Canvas2D;
use crate::tests::common::*;
use crate::ui::widgets::{Timeline, TimelineItem};
use crate::ui::WidgetRender;

fn render(timeline: &Timeline, canvas: &mut SharedRasterizer, frame: Rect) {
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
    WidgetRender::render(timeline, frame, &mut ctx, &tree);
}

#[test]
fn pending_row_participates_in_measurement() {
    let constraints = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let items = vec![TimelineItem::new("开始"), TimelineItem::new("完成")];

    assert_eq!(
        Timeline::new().items(items.clone()).measure(constraints),
        Size::new(400.0, 120.0)
    );
    assert_eq!(
        Timeline::new()
            .items(items)
            .pending(true)
            .measure(constraints),
        Size::new(400.0, 180.0)
    );
    assert_eq!(
        Timeline::new().pending(true).measure(constraints),
        Size::new(400.0, 60.0)
    );
    assert_eq!(Timeline::new().measure(constraints), Size::new(400.0, 60.0));
}

#[test]
fn pending_row_and_its_connector_render_inside_measured_frame() {
    let timeline = Timeline::new()
        .items(vec![TimelineItem::new("开始"), TimelineItem::new("完成")])
        .pending(true);
    let frame = Rect::new(0.0, 0.0, 400.0, 180.0);
    let mut canvas = SharedRasterizer::new(PixelSurface::new(400, 180));

    render(&timeline, &mut canvas, frame);

    let pixels = canvas.surface().pixels();
    assert!(
        (129..142).any(|y| pixels[y * 400 + 10..y * 400 + 23]
            .iter()
            .any(|pixel| *pixel != 0)),
        "pending dot must be visible"
    );
    assert!(
        pixels[105 * 400 + 14..105 * 400 + 19]
            .iter()
            .any(|pixel| *pixel != 0),
        "pending connector must join the previous row"
    );
}
