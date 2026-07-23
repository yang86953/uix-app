use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::native::test_harness::FakeClipboard;
use crate::tests::common::*;
use crate::ui::foundation::clipboard::with_clipboard;
use crate::ui::widgets::Typography;
use crate::ui::{AccessibilityRole, ComponentId};

fn render_typography(typography: &Typography, frame: Rect) -> Vec<u32> {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(320, 160));
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
        Orientation::YDown,
        320,
        160,
    );
    WidgetRender::render(typography, frame, &mut ctx, &tree);
    canvas.surface().pixels().to_vec()
}

#[test]
fn paragraph_measure_accounts_for_wrapped_lines() {
    let paragraph = Typography::paragraph("alpha beta gamma delta");
    let measured = paragraph.measure(Constraints::loose(Size::new(60.0, 300.0)));

    assert_eq!(measured.w, 60.0);
    assert!(measured.h > 20.0, "paragraph should wrap: {measured:?}");
}

#[test]
fn paragraph_measure_accounts_for_cjk_width_and_explicit_line_breaks() {
    let cjk = Typography::paragraph("界界界界界界界界");
    let cjk_size = cjk.measure(Constraints::loose(Size::new(28.0, 300.0)));
    assert_eq!(cjk_size, Size::new(28.0, 84.0));

    let multiline = Typography::paragraph("一\n二\n三");
    let multiline_size = multiline.measure(Constraints::loose(Size::new(200.0, 300.0)));
    assert_eq!(multiline_size.h, 63.0);
}

#[test]
fn paragraph_measure_matches_word_boundaries_and_normalized_line_breaks() {
    let words = Typography::paragraph("AAAA BBBB CCCC");
    let words_size = words.measure(Constraints::loose(Size::new(55.0, 300.0)));
    assert_eq!(words_size.h, 63.0);

    let mixed_breaks = Typography::paragraph("A\r\nB\rC");
    let mixed_breaks_size = mixed_breaks.measure(Constraints::loose(Size::new(200.0, 300.0)));
    assert_eq!(mixed_breaks_size.h, 63.0);
}

#[test]
fn paragraph_render_places_wrapped_glyphs_on_later_lines() {
    let pixels = render_typography(
        &Typography::paragraph("WWWWWWWW"),
        Rect::new(20.0, 20.0, 40.0, 100.0),
    );
    let later_line_pixels = pixels
        .iter()
        .enumerate()
        .filter(|(index, pixel)| **pixel != 0 && index / 320 >= 40)
        .count();

    assert!(
        later_line_pixels > 0,
        "wrapped paragraph must paint glyphs below the first line"
    );
}

#[test]
fn paragraph_hit_testing_preserves_the_character_boundary_of_an_empty_line() {
    let paragraph = Typography::paragraph("A\n\nB");
    let _ = render_typography(&paragraph, Rect::new(20.0, 20.0, 120.0, 100.0));

    assert_eq!(paragraph.cross_text_char_at(Point::new(1.0, 22.0)), 2);
}

#[test]
fn typography_style_flags_produce_visible_decoration_pixels() {
    let plain = render_typography(
        &Typography::text("Status"),
        Rect::new(20.0, 20.0, 160.0, 32.0),
    );
    let styled = render_typography(
        &Typography::text("Status")
            .mark()
            .underline()
            .delete()
            .strong(),
        Rect::new(20.0, 20.0, 160.0, 32.0),
    );
    let plain_pixels = plain.iter().filter(|pixel| **pixel != 0).count();
    let styled_pixels = styled.iter().filter(|pixel| **pixel != 0).count();

    assert!(
        styled_pixels > plain_pixels,
        "decorations should add pixels: plain={plain_pixels}, styled={styled_pixels}"
    );
}

#[test]
fn copyable_typography_supports_pointer_keyboard_and_semantics() {
    let mut typography = Typography::text("release notes").copyable(true);
    let _ = render_typography(&typography, Rect::new(80.0, 40.0, 180.0, 32.0));
    let copy = typography.copy_rect_for_test().expect("copy region");
    assert!(copy.x < 180.0, "copy region must use local coordinates");
    let click = SystemEvent::PointerDown {
        pos: Point::new(copy.x + copy.w * 0.5, copy.y + copy.h * 0.5),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    let mut clipboard = FakeClipboard::new();

    with_clipboard(&mut clipboard, || {
        assert_eq!(typography.on_event(&click), EventResult::Handled);
    });
    assert_eq!(clipboard.last_set_text(), Some("release notes"));
    assert_eq!(
        typography
            .semantic_event(ComponentId::new(12), &click)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("copied".to_string())
    );

    assert_eq!(
        typography.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    assert!(typography.is_copy_focused());
    clipboard.clear_history();
    let enter = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };
    with_clipboard(&mut clipboard, || {
        assert_eq!(typography.on_event(&enter), EventResult::Handled);
    });
    assert_eq!(clipboard.last_set_text(), Some("release notes"));

    let accessibility = typography.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.name.as_deref(), Some("release notes"));
}

#[test]
fn disabled_copyable_typography_does_not_copy() {
    let mut typography = Typography::text("secret").copyable(true).disabled(true);
    let mut clipboard = FakeClipboard::new();
    let enter = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };

    with_clipboard(&mut clipboard, || {
        assert_eq!(typography.on_event(&enter), EventResult::NotHandled);
    });
    assert_eq!(clipboard.last_set_text(), None);
}

#[test]
fn title_levels_drive_layout_paint_and_heading_semantics() {
    let constraints = Constraints::unconstrained();
    let mut previous_height = f32::INFINITY;
    let mut previous_paint: Option<Vec<u32>> = None;

    for level in 1..=5 {
        let title = Typography::title("Heading").level(level);
        let measured = title.measure(constraints);
        assert!(
            measured.h < previous_height,
            "h{level} should be smaller than the previous level: {measured:?}"
        );
        previous_height = measured.h;

        let painted = render_typography(&title, Rect::new(10.0, 10.0, 280.0, 60.0));
        if let Some(previous) = previous_paint.as_ref() {
            assert_ne!(
                &painted, previous,
                "h{level} should produce level-specific glyph geometry"
            );
        }
        previous_paint = Some(painted);

        let accessibility = title.snapshot_fields().accessibility();
        assert_eq!(accessibility.role, AccessibilityRole::Heading);
        assert!(accessibility
            .aria_attributes()
            .iter()
            .any(
                |attribute| attribute.name == "aria-level" && attribute.value == level.to_string()
            ));
    }

    let low = Typography::title("Low")
        .level(0)
        .snapshot_fields()
        .accessibility();
    let high = Typography::title("High")
        .level(u8::MAX)
        .snapshot_fields()
        .accessibility();
    assert!(low
        .aria_attributes()
        .iter()
        .any(|attribute| attribute.name == "aria-level" && attribute.value == "1"));
    assert!(high
        .aria_attributes()
        .iter()
        .any(|attribute| attribute.name == "aria-level" && attribute.value == "5"));
}

#[test]
fn paragraph_spacing_and_indent_change_measured_and_rendered_geometry() {
    let default = Typography::paragraph("one\ntwo\nthree");
    let spaced = Typography::paragraph("one\ntwo\nthree").spacing(2.0);
    let constraints = Constraints::loose(Size::new(240.0, 300.0));
    assert_eq!(default.measure(constraints).h, 63.0);
    assert_eq!(spaced.measure(constraints).h, 84.0);

    let _ = render_typography(&spaced, Rect::new(0.0, 0.0, 240.0, 100.0));
    let spaced_lines = spaced.rendered_line_origins_for_test();
    assert_eq!(spaced_lines.len(), 3);
    assert!((spaced_lines[1].y - spaced_lines[0].y - 28.0).abs() < 0.01);

    let indented = Typography::paragraph("WWWWWWWWWWWW")
        .spacing(1.5)
        .indent(2.0);
    let _ = render_typography(&indented, Rect::new(0.0, 0.0, 90.0, 120.0));
    let origins = indented.rendered_line_origins_for_test();
    assert!(
        origins.len() >= 2,
        "indented paragraph should wrap: {origins:?}"
    );
    assert!((origins[0].x - 28.0).abs() < 0.01, "{origins:?}");
    assert!(
        origins[1].x.abs() < 0.01,
        "only the first line is indented: {origins:?}"
    );

    let plain_width = Typography::paragraph("short").measure(constraints).w;
    let indented_width = Typography::paragraph("short")
        .indent(2.0)
        .measure(constraints)
        .w;
    assert!(indented_width > plain_width);

    for paragraph in [
        Typography::paragraph("safe")
            .spacing(f32::NAN)
            .indent(f32::INFINITY),
        Typography::paragraph("safe")
            .spacing(f32::MAX)
            .indent(f32::MAX),
        Typography::paragraph("safe").spacing(-1.0).indent(-1.0),
    ] {
        let measured = paragraph.measure(constraints);
        assert!(
            measured.w.is_finite() && measured.h.is_finite(),
            "{measured:?}"
        );
    }
}

#[test]
fn text_keeps_single_line_inline_geometry_and_text_semantics() {
    let inline = Typography::text("inline text that exceeds its frame")
        .spacing(3.0)
        .indent(2.0);
    let _ = render_typography(&inline, Rect::new(0.0, 0.0, 32.0, 24.0));
    let origins = inline.rendered_line_origins_for_test();
    assert_eq!(origins.len(), 1);
    assert!(
        origins[0].x.abs() < 0.01,
        "paragraph indent must not affect inline text"
    );

    let accessibility = inline.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Text);
    assert!(!accessibility
        .aria_attributes()
        .iter()
        .any(|attribute| attribute.name == "aria-level"));
}

#[test]
fn typography_reconcile_updates_level_and_paragraph_geometry() {
    let constraints = Constraints::loose(Size::new(120.0, 300.0));
    let mut typography = Typography::title("Release").level(1);
    let h1 = typography.measure(constraints);

    typography.sync_from(Typography::title("Release").level(5));
    let h5 = typography.measure(constraints);
    assert!(h5.h < h1.h);
    let accessibility = typography.snapshot_fields().accessibility();
    assert!(accessibility
        .aria_attributes()
        .iter()
        .any(|attribute| attribute.name == "aria-level" && attribute.value == "5"));

    typography.sync_from(Typography::paragraph("one\ntwo").spacing(2.0).indent(1.0));
    assert_eq!(typography.measure(constraints).h, 56.0);
    assert_eq!(
        typography.snapshot_fields().accessibility().role,
        AccessibilityRole::Text
    );
}
