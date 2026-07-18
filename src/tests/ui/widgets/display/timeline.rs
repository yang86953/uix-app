use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::draw::traits::canvas::Canvas2D;
use crate::tests::common::*;
use crate::ui::widgets::{Timeline, TimelineItem};
use crate::ui::{AccessibilityRole, WidgetRender};

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

fn render_display_list(timeline: &Timeline, frame: Rect, surface_size: (i32, i32)) -> String {
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
            WidgetRender::render(timeline, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
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

#[test]
fn constrained_timeline_clips_and_elides_each_visible_row() {
    let timeline = Timeline::new()
        .items(vec![
            TimelineItem::new("第一条很长的中英文混合时间轴标题 mixed value")
                .description("第一条同样很长的说明 description value"),
            TimelineItem::new("第二条很长的时间轴标题").description("第二条很长的说明"),
            TimelineItem::new("第三条很长的时间轴标题").description("第三条很长的说明"),
        ])
        .pending(true);
    let display_list =
        render_display_list(&timeline, Rect::new(10.0, 6.0, 132.0, 120.0), (160, 140));

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 6.0, w: 132.0, h: 120.0 } }"),
        "{display_list}"
    );
    assert!(display_list.contains('…'), "{display_list}");
    assert!(!display_list.contains("NaN"), "{display_list}");
    assert!(
        !display_list.contains("w: -") && !display_list.contains("h: -"),
        "{display_list}"
    );

    let invalid = render_display_list(
        &Timeline::new().add(TimelineItem::new("invalid")),
        Rect::new(0.0, 0.0, f32::NAN, -10.0),
        (20, 20),
    );
    assert_eq!(invalid, "DisplayList { ops: [] }");
}

#[test]
fn timeline_accessibility_follows_visual_order_and_includes_pending() {
    let timeline = Timeline::new()
        .items(vec![
            TimelineItem::new("开始").description("alpha"),
            TimelineItem::new("完成"),
        ])
        .pending(true)
        .reverse(true);
    let accessibility = timeline.snapshot_fields().accessibility();
    let pending = crate::ui::locale::use_locale().timeline_pending;

    assert_eq!(accessibility.role, AccessibilityRole::List);
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some(format!("完成; 开始: alpha; {pending}").as_str())
    );

    let empty = Timeline::new().snapshot_fields().accessibility();
    assert_eq!(empty.role, AccessibilityRole::List);
    assert_eq!(empty.state.value_text, None);
}
