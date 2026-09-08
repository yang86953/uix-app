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
            "Fixture Noto",
            include_bytes!("../../../../assets/fonts/NotoSansCJKsc-Regular.otf"),
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
            source, width, |text| actual(&fonts, text, 14.0, 0.0).0,
        );
        if let Some(value) = value {
            assert!(!value.contains(['\r', '\n']));
            assert!(actual(&fonts, &value, 14.0, 0.0).0 <= width);
            if width == 95.0 { assert!(value.ends_with("…")); }
        } else { assert_eq!(width, 1.0); }
    }
}
