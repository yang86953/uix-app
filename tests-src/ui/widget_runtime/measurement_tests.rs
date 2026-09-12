use std::rc::Rc;

use super::LayoutFontScope;
use crate::core::{Constraints, Size};
use crate::draw::resources::font::text_backend::TextLayoutOptions;
use crate::draw::{FontBundle, FontService, HAlign, VAlign};
use crate::ui::theme::style::{LineHeight, Style, TypographyToken};
use crate::ui::widget_runtime::{dynamic_label::DynamicLabel, traits::WidgetLayout};
use crate::ui::widgets::Label;

fn fonts() -> Rc<FontService> {
    let mut fonts = FontService::new();
    fonts
        .install_font_bundle(&FontBundle::from_static(
            "UIX Test Body",
            include_bytes!("../../../tests/fixtures/fonts/uix-test-body.ttf"),
        ))
        .unwrap();
    Rc::new(fonts)
}

fn actual(fonts: &FontService, text: &str, size: f32, width: f32) -> (f32, usize) {
    let layout = fonts.layout_text_shared(
        &fonts.loaded_font_handle,
        text,
        &TextLayoutOptions {
            max_width: width,
            max_height: 0.0,
            font_size: size,
            line_height: size * 1.5,
            word_wrap: width > 0.0,
            h_align: HAlign::Left,
            v_align: VAlign::Top,
        },
    );
    (layout.width, layout.lines.len().max(1))
}

fn dynamic(text: &str, size: f32) -> DynamicLabel {
    let text = text.to_owned();
    let mut label = DynamicLabel::new(move || text.clone());
    let mut style = Style::default();
    style.font_size = TypographyToken::Custom(size);
    label.set_style(style);
    label
}

#[test]
fn intrinsic_text_width_fits_real_shaping_for_counts_and_names() {
    let fonts = fonts();
    let _scope = LayoutFontScope::enter(Rc::clone(&fonts));
    let constraints = Constraints::new(Size::zero(), Size::new(1200.0, 1200.0), None);
    for text in [
        "11",
        "77",
        "123456",
        "smc",
        "rust-game-common",
        "9月8日，星期二",
        "中文 Hg 🙂",
    ] {
        for size in [12.0, 14.0, 18.5, 30.0] {
            let real = actual(&fonts, text, size, 0.0).0;
            for measured in [
                dynamic(text, size).measure(constraints),
                Label::new(text).font_size(size).measure(constraints),
            ] {
                assert!(
                    measured.w >= real && measured.w - real < 1.01,
                    "{text:?} / {size}px: allocated {} < shaped {real}",
                    measured.w
                );
                assert_eq!(
                    actual(&fonts, text, size, measured.w).1,
                    1,
                    "{text:?} wrapped in its own hug width"
                );
                assert_eq!(measured.h, size * 1.5);
            }
        }
    }
}

#[test]
fn constrained_dynamic_text_uses_real_wrapped_height() {
    let fonts = fonts();
    let _scope = LayoutFontScope::enter(Rc::clone(&fonts));
    let text = "11111111111111 中文 English 接续内容";
    let size = 18.0;
    for width in [42.0, 80.0, 200.0] {
        let measured = dynamic(text, size).measure(Constraints::new(
            Size::zero(),
            Size::new(width, 1200.0),
            None,
        ));
        let lines = actual(&fonts, text, size, width).1;
        assert!(lines > 1);
        assert_eq!(measured.w, width);
        assert_eq!(measured.h, lines as f32 * size * 1.5);
    }
}

#[test]
fn font_scope_restores_after_nested_empty_font_service_and_unwind() {
    let constraints = Constraints::new(Size::zero(), Size::new(500.0, 500.0), None);
    let text = dynamic("11", 30.0);
    let fallback = text.measure(constraints).w;
    let fonts = fonts();
    {
        let _scope = LayoutFontScope::enter(Rc::clone(&fonts));
        let exact = text.measure(constraints).w;
        assert!(exact > fallback);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _inner = LayoutFontScope::enter(Rc::new(FontService::new()));
            assert_eq!(text.measure(constraints).w, fallback);
            panic!("scope unwind probe");
        }));
        assert_eq!(text.measure(constraints).w, exact);
    }
    assert_eq!(text.measure(constraints).w, fallback);
}

#[test]
fn explicit_newlines_empty_text_padding_and_line_height_remain_stable() {
    let fonts = fonts();
    let _scope = LayoutFontScope::enter(fonts);
    let mut label = dynamic("11\r\n77\n", 20.0);
    let mut style = Style::default();
    style.font_size = TypographyToken::Custom(20.0);
    style.line_height = LineHeight::factor(2.0);
    style.padding = crate::core::EdgeInsets::uniform(5.0);
    label.set_style(style);
    let c = Constraints::new(Size::zero(), Size::new(500.0, 500.0), None);
    assert_eq!(label.measure(c).h, 130.0);
    assert_eq!(dynamic("", 20.0).measure(c).h, 30.0);
}

#[test]
fn elided_label_keeps_single_line_height_and_full_semantics() {
    let fonts = fonts();
    let _scope = LayoutFontScope::enter(Rc::clone(&fonts));
    let source = "computer-control-toolkit\n中文";
    let label = dynamic(source, 14.0).elided();
    let bounds = Constraints::new(Size::zero(), Size::new(95.0, 500.0), None);
    assert_eq!(label.measure(bounds), Size::new(95.0, 21.0));
    assert_eq!(label.semantic_text(), source);
    for width in [1.0, 30.0, 95.0, 500.0] {
        let value = crate::ui::widget_runtime::paint_context::elide_single_line_cow_by(
            source,
            width,
            |text| actual(&fonts, text, 14.0, 0.0).0,
        );
        if let Some(value) = value {
            assert!(!value.contains(['\r', '\n']));
            assert!(actual(&fonts, &value, 14.0, 0.0).0 <= width);
            if width == 95.0 {
                assert!(value.ends_with("…"));
            }
        } else {
            assert_eq!(width, 1.0);
        }
    }
}

#[test]
fn static_and_dynamic_wrap_at_parent_limit_even_with_larger_requested_width() {
    let fonts = fonts();
    let _scope = LayoutFontScope::enter(Rc::clone(&fonts));
    let source = "从代码事实生成模块、组件与依赖；先预览，再由人确认合入。";
    let mut style = Style::default();
    style.font_size = TypographyToken::Custom(12.0);
    style.width = Some(900.0);
    style.padding = crate::core::EdgeInsets::uniform(6.0);
    style.line_height = LineHeight::pixels(24.0);
    let literal = Label::new(source).word_wrap(true).style(style.clone());
    let mut dynamic = dynamic(source, 12.0);
    dynamic.set_style(style);
    for width in [12.0, 60.0, 120.0, 200.0] {
        let bounds = Constraints::new(Size::zero(), Size::new(width, 1200.0), None);
        let static_size = literal.measure(bounds);
        let dynamic_size = dynamic.measure(bounds);
        assert_eq!(static_size, dynamic_size);
        assert_eq!(static_size.w, width);
        if width > 12.0 {
            assert_eq!(
                static_size.h,
                actual(&fonts, source, 12.0, width - 12.0).1 as f32 * 24.0 + 12.0
            );
        }
    }
    let capped = Constraints::new(Size::zero(), Size::new(60.0, 30.0), None);
    assert_eq!(literal.measure(capped), Size::new(60.0, 30.0));
    assert_eq!(dynamic.measure(capped), Size::new(60.0, 30.0));
    assert_eq!(
        Label::new(source).font_size(12.0).measure(capped),
        Size::new(60.0, 18.0)
    );
}

#[test]
fn text_pixels_cannot_escape_allocated_content_rect_or_zero_size() {
    use crate::core::Rect;
    use crate::draw::painting::{PaintContext as DrawContext, PaintSurfaceConfig};
    use crate::draw::raster::{pixel_surface::PixelSurface, shared_rasterizer::SharedRasterizer};
    use crate::draw::{Color, ImageService};
    use crate::ui::WidgetTree;
    use crate::ui::theme::Theme;
    use crate::ui::widget_runtime::paint_context::PaintContext;
    use crate::ui::widget_runtime::traits::WidgetRender;

    let fonts = fonts();
    let images = ImageService::new();
    let text = "从代码事实生成模块、组件与依赖；先预览，再由人确认合入。";
    let mut style = Style::default();
    style.font_size = TypographyToken::Custom(20.0);
    style.padding = crate::core::EdgeInsets::uniform(5.0);
    let literal = Label::new(text).word_wrap(true).style(style.clone());
    let nowrap = Label::new(text).style(style.clone());
    let mut dynamic = dynamic(text, 20.0);
    dynamic.set_style(style);
    for widget in [&literal as &dyn WidgetRender, &dynamic, &nowrap] {
        for (width, height) in [(90.0, 30.0), (0.0, 30.0), (90.0, 0.0), (8.0, 8.0)] {
            let frame = Rect::new(20.0, 20.0, width, height);
            let mut surface = PixelSurface::new(200, 160);
            surface.set_clear_color(Color::WHITE);
            surface.clear_all();
            let mut canvas = SharedRasterizer::new(surface);
            {
                let mut draw = DrawContext::new(
                    &mut canvas,
                    fonts.loaded_font_handle,
                    &fonts,
                    &images,
                    PaintSurfaceConfig {
                        dpi: 96.0,
                        device_pixel_ratio: 1.0,
                        orientation: Default::default(),
                        surface_w: 200,
                        surface_h: 160,
                    },
                );
                let mut ctx = PaintContext::new(&mut draw, Theme::antd_light().tokens_arc());
                widget.render(frame, &mut ctx, &WidgetTree::new());
                // 必须弹出内部 clip，不能污染后继节点。
                ctx.fill_rect(Rect::new(180.0, 140.0, 5.0, 5.0), Color::BLACK, None);
            }
            let mut ink = 0;
            for (index, pixel) in canvas.surface().pixels().iter().enumerate() {
                let (x, y) = ((index % 200) as f32, (index / 200) as f32);
                if (180.0..185.0).contains(&x) && (140.0..145.0).contains(&y) {
                    assert_eq!(*pixel, 0xff000000, "clip leaked into following widget");
                } else if *pixel != 0xffffffff {
                    ink += 1;
                    assert!(
                        x >= 25.0 && x < 20.0 + width - 5.0 && y >= 25.0 && y < 20.0 + height - 5.0,
                        "paint outside {frame:?}: {x},{y}"
                    );
                }
            }
            if width == 90.0 && height == 30.0 {
                assert!(ink > 20);
            } else {
                assert_eq!(ink, 0);
            }
        }
    }
}
