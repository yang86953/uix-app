use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::display::descriptions::*;
use crate::ui::AccessibilityRole;

fn render_descriptions_in(
    descriptions: &Descriptions,
    frame: Rect,
    surface_size: (i32, i32),
) -> String {
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
            WidgetRender::render(descriptions, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

#[test]
fn descriptions_normalizes_zero_columns_before_measurement() {
    let descriptions = Descriptions::new()
        .items(vec![
            DescriptionsItem::new("Name", "Ada"),
            DescriptionsItem::new("Role", "Engineer"),
        ])
        .column(0);

    assert!(matches!(
        descriptions.snapshot_fields(),
        SnapshotFields::Descriptions { column: 1, .. }
    ));
    assert_eq!(
        descriptions.measure(Constraints::loose(Size::new(600.0, 160.0))),
        Size::new(600.0, 72.0)
    );
}

#[test]
fn descriptions_packs_spans_without_overlap_and_measures_every_row() {
    let constraints = Constraints::loose(Size::new(600.0, 300.0));
    let descriptions = Descriptions::new()
        .items(vec![
            DescriptionsItem::new("Name", "Ada").span(2),
            DescriptionsItem::new("Role", "Engineer").span(2),
            DescriptionsItem::new("Team", "Compiler"),
        ])
        .column(3);

    assert_eq!(
        descriptions.measure(constraints),
        Size::new(600.0, 72.0),
        "the second span=2 item must wrap instead of overlapping column three"
    );
}

#[test]
fn descriptions_clamps_invalid_spans_and_label_width() {
    let constraints = Constraints::loose(Size::new(600.0, 300.0));
    let descriptions = Descriptions::new()
        .items(vec![
            DescriptionsItem::new("Wide", "value").span(usize::MAX),
            DescriptionsItem::new("Zero", "value").span(0),
        ])
        .column(2)
        .label_width(f32::NAN);

    assert_eq!(descriptions.measure(constraints), Size::new(600.0, 72.0));
    assert!(matches!(
        descriptions.snapshot_fields(),
        SnapshotFields::Descriptions {
            label_width: 0.0,
            ref items,
            ..
        } if items[1].span == 1
    ));
}

#[test]
fn descriptions_reflows_columns_and_wraps_long_values_at_narrow_widths() {
    let descriptions = Descriptions::new()
        .title("验收信息")
        .items(vec![
            DescriptionsItem::new("负责人", "贝露丹迪"),
            DescriptionsItem::new(
                "说明",
                "这是一段需要在窄宽度下自动换行并增加描述列表高度的中文正文。",
            ),
            DescriptionsItem::new("状态", "执行中"),
        ])
        .column(3);

    let wide = descriptions.measure(Constraints::loose(Size::new(600.0, 600.0)));
    let narrow = descriptions.measure(Constraints::loose(Size::new(220.0, 600.0)));

    assert_eq!(wide.w, 600.0);
    assert_eq!(narrow.w, 220.0);
    assert!(
        narrow.h > wide.h,
        "narrow rows must reflow and wrap: {narrow:?} <= {wide:?}"
    );
}

#[test]
fn descriptions_render_clips_elides_title_and_wraps_cells() {
    let descriptions = Descriptions::new()
        .title("很长的描述列表标题需要留在组件边界内")
        .items(vec![
            DescriptionsItem::new("很长的字段标签", "字段值也需要根据真实宽度自动换行"),
            DescriptionsItem::new("状态", "执行中"),
        ])
        .bordered(true)
        .column(2);

    let display_list = render_descriptions_in(
        &descriptions,
        Rect::new(10.0, 5.0, 180.0, 120.0),
        (220, 150),
    );

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 5.0, w: 180.0, h: 120.0 } }"),
        "Descriptions must clip all paint to its frame: {display_list}"
    );
    assert!(
        display_list.contains('…'),
        "long title must elide: {display_list}"
    );
    assert!(
        display_list.contains("DrawTextWrapped"),
        "labels and values must use wrapped text: {display_list}"
    );
    assert!(
        !display_list.contains("w: -") && !display_list.contains("h: -"),
        "{display_list}"
    );
}

#[test]
fn descriptions_accessibility_exposes_title_and_read_only_values() {
    let fields = Descriptions::new()
        .title("账户信息")
        .items(vec![
            DescriptionsItem::new("姓名", "Ada"),
            DescriptionsItem::new("角色", "Engineer"),
        ])
        .snapshot_fields();
    let accessibility = fields.accessibility();

    assert_eq!(accessibility.role, AccessibilityRole::Group);
    assert_eq!(accessibility.name.as_deref(), Some("账户信息"));
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("姓名: Ada; 角色: Engineer")
    );
}
