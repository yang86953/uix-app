use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::display::empty::*;
use crate::ui::AccessibilityRole;

fn render_empty_in(empty: &Empty, frame: Rect, surface_size: (i32, i32)) -> String {
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
            WidgetRender::render(empty, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

#[test]
fn empty_measures_wrapped_description_from_the_constrained_width() {
    let empty = Empty::new()
        .description("这是一段很长的空状态说明，需要根据真实可用宽度自动换行并增加组件高度。");

    let wide = empty.measure(Constraints::loose(Size::new(320.0, 500.0)));
    let narrow = empty.measure(Constraints::loose(Size::new(120.0, 500.0)));

    assert_eq!(wide.w, 320.0);
    assert_eq!(narrow.w, 120.0);
    assert!(
        narrow.h > wide.h,
        "narrow description must wrap: {narrow:?} <= {wide:?}"
    );
}

#[test]
fn empty_render_clips_icon_and_wraps_long_description() {
    let empty = Empty::new()
        .icon("inbox")
        .description("窄宽度下的空状态说明需要自动换行，并始终留在组件边界内。");
    let display_list = render_empty_in(&empty, Rect::new(10.0, 5.0, 120.0, 96.0), (160, 120));

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 5.0, w: 120.0, h: 96.0 } }"),
        "Empty must clip all paint to its frame: {display_list}"
    );
    assert!(
        display_list.contains("DrawTextWrapped"),
        "long description must use wrapped text: {display_list}"
    );
    assert!(
        !display_list.contains("w: -") && !display_list.contains("h: -"),
        "{display_list}"
    );
}

#[test]
fn text_only_empty_uses_centered_text_without_an_empty_icon_slot() {
    let empty = Empty::new().description("暂无结果");
    let display_list = render_empty_in(&empty, Rect::new(0.0, 0.0, 160.0, 100.0), (180, 120));

    assert!(display_list.contains("TextCenter"), "{display_list}");
    assert!(
        !display_list.contains("\u{e129}") && !display_list.contains("\u{e151}"),
        "text-only Empty must not invent an icon: {display_list}"
    );
}

#[test]
fn empty_accessibility_exposes_the_description_as_status() {
    let fields = Empty::new()
        .description("当前筛选条件下没有结果")
        .snapshot_fields();
    let accessibility = fields.accessibility();

    assert_eq!(accessibility.role, AccessibilityRole::Status);
    assert_eq!(
        accessibility.name.as_deref(),
        Some("当前筛选条件下没有结果")
    );
}
