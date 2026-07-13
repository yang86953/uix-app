use super::*;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::font::font_service::FontService;
use crate::draw::image::ImageService;
use crate::draw::painting::{DisplayList, PaintContext};
use crate::draw::spatial::Orientation;
use crate::native::test_harness::FakeClipboard;
use crate::native::traits::input::IClipboard;
use crate::ui::theme::DesignTokens;
use crate::ui::traits::{EventHandler, WidgetComponent, WidgetLayout, WidgetRender};

fn install_clipboard(clipboard: &mut FakeClipboard) {
    let c: &mut dyn IClipboard = clipboard;
    let wide: *mut dyn IClipboard = c;
    let parts: (usize, usize) = unsafe { std::mem::transmute(wide) };
    clipboard::set_clipboard_parts(parts.0, parts.1);
}

fn clear_clipboard() {
    clipboard::set_clipboard_parts(0, 0);
}

#[test]
fn sync_from_preserves_runtime_value() {
    let mut input = Input::new("old").with_value("kept");
    input.cursor_char = 2;
    input.sync_from(Input::new("new"));
    assert_eq!(input.value(), "kept");
    assert_eq!(input.placeholder, "new");
    assert_eq!(input.cursor_char, 2);
}

#[test]
fn ctrl_v_pastes_from_injected_clipboard() {
    let mut clipboard = FakeClipboard::new();
    clipboard.set_text("clip");
    install_clipboard(&mut clipboard);

    let mut input = Input::new("").with_value("ab");
    input.cursor_char = 1;
    let result = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::V,
        mods: KeyMod::CTRL,
    });

    assert_eq!(result, EventResult::Handled);
    assert_eq!(input.value(), "aclipb");
    clear_clipboard();
}

#[test]
fn paste_event_replaces_selection() {
    let mut input = Input::new("").with_value("abcd");
    input.set_selection_range(1, 3);
    input.cursor_char = 3;

    let result = input.on_event(&SystemEvent::Paste {
        text: "XY".to_string(),
    });

    assert_eq!(result, EventResult::Handled);
    assert_eq!(input.value(), "aXYd");
}

#[test]
fn measure_clamps_input_size() {
    let measured = Input::new("Search").measure(Constraints::loose(Size::new(60.0, 20.0)));

    assert_eq!(measured, Size::new(60.0, 20.0));
}

#[test]
fn focus_and_ime_events_keep_preedit_separate_from_committed_value() {
    let mut input = Input::new("type here");

    assert_eq!(input.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert!(input.focused);
    assert_eq!(
        input.on_event(&SystemEvent::ImeCompositionStart),
        EventResult::Handled
    );
    assert_eq!(
        input.on_event(&SystemEvent::ImeCompositionUpdate {
            text: "zhong".to_string(),
        }),
        EventResult::Handled
    );
    assert_eq!(input.composition, "zhong");
    assert_eq!(input.value(), "");

    assert_eq!(
        input.on_event(&SystemEvent::ImeCompositionEnd {
            text: "中".to_string(),
        }),
        EventResult::Handled
    );
    assert!(input.composition.is_empty());
    assert_eq!(input.value(), "");
    assert_eq!(
        input.on_event(&SystemEvent::TextInput {
            text: "中".to_string(),
        }),
        EventResult::Handled
    );
    assert_eq!(input.value(), "中");
}

#[test]
fn text_input_capability_reports_enabled_state_and_caret_rect() {
    let input = Input::new("type here");
    input.caret_rect.set(Rect::new(10.0, 20.0, 1.5, 18.0));

    assert!(input
        .capabilities()
        .contains(crate::ui::traits::WidgetCapabilities::TEXT_INPUT));
    let client = input.as_text_input().expect("Input text capability");
    assert!(client.accepts_text_input());
    assert_eq!(
        client.text_input_cursor_rect(),
        Rect::new(10.0, 20.0, 1.5, 18.0)
    );

    let disabled = Input::new("type here").disabled(true);
    assert!(!disabled
        .as_text_input()
        .expect("Input text capability")
        .accepts_text_input());
}

#[test]
fn translated_software_render_keeps_placeholder_and_value_glyphs_visible() {
    fn render(input: Option<&Input>, replay: Option<&DisplayList>) -> (Vec<u32>, DisplayList) {
        let mut canvas = SharedRasterizer::new(PixelSurface::new(120, 40));
        let mut fonts = FontService::new();
        let font = fonts
            .load_font(include_bytes!("../../../../../../assets/fonts/lucide.ttf"))
            .expect("load deterministic test font");
        let images = ImageService::new();
        let tokens = DesignTokens::antd_light();
        let tree = WidgetTree::new();
        let frame = Rect::new(240.0, 160.0, 100.0, 32.0);
        let mut list = DisplayList::new();

        {
            let mut ctx = PaintContext::new(
                &mut canvas,
                font,
                &fonts,
                &images,
                &tokens,
                96.0,
                1.0,
                Orientation::YDown,
                120,
                40,
            );
            ctx.canvas_2d().translate(-frame.x, -frame.y);
            if let Some(input) = input {
                ctx.set_recorder(Some(&mut list));
                WidgetRender::render(input, frame, &mut ctx, &tree);
                ctx.set_recorder(None);
                assert!(ctx.recording_complete());
            } else if let Some(replay) = replay {
                replay.replay(&mut ctx);
            }
        }

        (canvas.surface().pixels().to_vec(), list)
    }

    const GLYPH: &str = "\u{E151}";
    let (baseline, _) = render(Some(&Input::new("")), None);
    let (placeholder, placeholder_list) = render(Some(&Input::new(GLYPH)), None);
    let (committed, committed_list) = render(Some(&Input::new("").with_value(GLYPH)), None);
    let mut focused_empty = Input::new("");
    focused_empty.on_event(&SystemEvent::FocusIn);
    let (focused_empty_pixels, _) = render(Some(&focused_empty), None);
    let mut preedit = Input::new("");
    preedit.on_event(&SystemEvent::FocusIn);
    preedit.on_event(&SystemEvent::ImeCompositionStart);
    preedit.on_event(&SystemEvent::ImeCompositionUpdate {
        text: GLYPH.to_string(),
    });
    let (preedit_pixels, preedit_list) = render(Some(&preedit), None);
    let changed = |pixels: &[u32]| {
        pixels
            .iter()
            .zip(&baseline)
            .filter(|(actual, base)| actual != base)
            .count()
    };

    assert!(
        changed(&placeholder) > 0,
        "placeholder glyph must reach pixels"
    );
    assert!(changed(&committed) > 0, "committed glyph must reach pixels");
    assert_ne!(
        preedit_pixels, focused_empty_pixels,
        "preedit glyph and underline must reach pixels"
    );
    assert!(
        preedit.caret_rect.get().x > 240.0 + PAD,
        "preedit caret must follow the composed glyph"
    );
    assert_eq!(
        render(None, Some(&placeholder_list)).0,
        placeholder,
        "cached replay must preserve placeholder glyph and clip"
    );
    assert_eq!(
        render(None, Some(&committed_list)).0,
        committed,
        "cached replay must preserve committed glyph and clip"
    );
    assert_eq!(
        render(None, Some(&preedit_list)).0,
        preedit_pixels,
        "cached replay must preserve preedit glyph, underline, and caret"
    );
}
