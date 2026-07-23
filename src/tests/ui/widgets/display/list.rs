use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::view::View;
use crate::ui::widgets::{Empty, List};
use crate::ui::AccessibilityRole;

fn render_display_list(list: &List, frame: Rect, surface_size: (i32, i32)) -> String {
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
            WidgetRender::render(list, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

#[test]
fn constrained_list_clips_rows_and_elides_long_single_line_content() {
    let list = List::new()
        .header("这是一段很长的质量核验清单标题")
        .items(vec![
            "第一项包含很长的中英文 mixed content and identifier",
            "第二项继续验证窄宽度下不会覆盖相邻内容",
        ])
        .footer("长页脚也必须保持在边界内")
        .load_more("加载更多质量检查项");
    let display_list = render_display_list(&list, Rect::new(10.0, 8.0, 150.0, 120.0), (180, 150));

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 8.0, w: 150.0, h: 120.0 } }"),
        "all list paint must be clipped to the actual frame: {display_list}"
    );
    assert!(
        display_list.contains('…'),
        "long fixed-height rows must use an ellipsis: {display_list}"
    );
    assert!(
        !display_list.contains("w: -") && !display_list.contains("h: -"),
        "{display_list}"
    );
}

#[test]
fn narrow_or_invalid_list_frames_do_not_emit_negative_geometry() {
    let list = List::new().items(vec!["内容"]);
    for frame in [
        Rect::new(0.0, 0.0, 8.0, 20.0),
        Rect::new(0.0, 0.0, f32::NAN, -10.0),
    ] {
        let display_list = render_display_list(&list, frame, (40, 40));
        assert!(!display_list.contains("NaN"), "{display_list}");
        assert!(
            !display_list.contains("w: -") && !display_list.contains("h: -"),
            "{display_list}"
        );
    }
}

#[test]
fn list_accessibility_exposes_header_and_static_item_text() {
    let accessibility = List::new()
        .header("质量清单")
        .items(vec!["视觉基线", "语义断言", "交互回归"])
        .snapshot_fields()
        .accessibility();

    assert_eq!(accessibility.role, AccessibilityRole::List);
    assert_eq!(accessibility.name.as_deref(), Some("质量清单"));
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("视觉基线; 语义断言; 交互回归")
    );
}

#[test]
fn empty_list_builds_the_default_empty_view_without_a_configured_factory() {
    let view = List::new().build();
    assert_eq!(view.widget_type_id(), std::any::TypeId::of::<Empty>());
}

#[test]
fn list_size_applies_to_header_items_and_footer_measurement() {
    let constraints = Constraints::loose(Size::new(600.0, 600.0));
    let items = vec!["Ada", "Alan"];

    let small = List::new()
        .header("People")
        .items(items.clone())
        .footer("2 people")
        .size(ControlSize::Small);
    assert_eq!(small.measure(constraints), Size::new(400.0, 128.0));

    let large = List::new()
        .header("People")
        .items(items)
        .footer("2 people")
        .size(ControlSize::Large);
    assert_eq!(large.measure(constraints), Size::new(400.0, 192.0));
}

#[test]
fn load_more_keeps_its_independent_action_slot_height() {
    let constraints = Constraints::loose(Size::new(600.0, 600.0));
    let list = List::new()
        .header("People")
        .items(vec!["Ada", "Alan"])
        .footer("2 people")
        .load_more("Load more")
        .size(ControlSize::Large);

    assert_eq!(list.measure(constraints), Size::new(400.0, 232.0));
}
