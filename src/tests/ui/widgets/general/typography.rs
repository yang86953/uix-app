use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
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
