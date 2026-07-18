use crate::tests::common::*;
use crate::ui::widgets::general::label::*;

fn render_label(label: &Label) -> Vec<u32> {
    let mut canvas = crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer::new(
        crate::draw::engine::cpu::pixel_surface::PixelSurface::new(120, 48),
    );
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        font,
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        crate::draw::spatial::Orientation::YDown,
        120,
        48,
    );
    WidgetRender::render(label, Rect::new(8.0, 8.0, 100.0, 24.0), &mut ctx, &tree);
    canvas.surface().pixels().to_vec()
}

#[test]
fn measure_clamps_label_size() {
    let measured = Label::new("abcdef").measure(Constraints::loose(Size::new(30.0, 12.0)));

    assert_eq!(measured, Size::new(30.0, 12.0));
}

#[test]
fn label_ignores_pointer_when_not_selectable() {
    let mut label = Label::new("首页");
    let down = SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(label.on_event(&down), EventResult::NotHandled);
    assert_eq!(label.selected_text(), None);
}

#[test]
fn selectable_label_handles_pointer_down() {
    let mut label = Label::new("首页").selectable();
    let down = SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(label.on_event(&down), EventResult::Handled);
}

#[test]
fn multiline_label_measure_covers_every_rendered_line_box() {
    let label = Label::new("一\n二\n三").font_size(12.0);
    let measured = label.measure(Constraints::unconstrained());

    assert_eq!(measured, Size::new(12.0, 54.0));
}

#[test]
fn label_measure_uses_wide_cjk_scalars_and_style_font_tokens() {
    let plain = Label::new("界界").font_size(12.0);
    let plain_size = plain.measure(Constraints::unconstrained());
    assert_eq!(plain_size.w, 24.0);
    assert!((plain_size.h - 14.4).abs() < 0.001);

    let styled = Label::new("界").style(Style::default());
    let styled_size = styled.measure(Constraints::unconstrained());
    assert_eq!(styled_size.w, 14.0);
    assert!((styled_size.h - 16.8).abs() < 0.001);
}

#[test]
fn label_measure_normalizes_invalid_and_physical_font_sizes() {
    for invalid in [f32::NAN, f32::INFINITY, 0.0, -4.0] {
        let size = Label::new("界")
            .font_size(invalid)
            .measure(Constraints::unconstrained());
        assert_eq!(size.w, 12.0);
        assert!((size.h - 14.4).abs() < 0.001);
    }

    let physical = Label::new("界")
        .font_size_unit(crate::draw::spatial::PhysicalUnit::Px(20.0))
        .measure(Constraints::unconstrained());
    assert_eq!(physical.w, 20.0);
    assert!((physical.h - 24.0).abs() < 0.001);

    let invalid_style = Label::new("界").style(Style::default().with_font_size(f32::NAN));
    let styled_size = invalid_style.measure(Constraints::unconstrained());
    assert_eq!(styled_size.w, 12.0);
    assert!((styled_size.h - 14.4).abs() < 0.001);
}

#[test]
fn label_render_normalizes_an_invalid_public_font_size() {
    let valid = render_label(&Label::new("Status"));
    let mut invalid = Label::new("Status");
    invalid.font_size = f32::NAN;

    assert_eq!(render_label(&invalid), valid);
}
