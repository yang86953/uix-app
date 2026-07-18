use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::painting::DisplayList;
use crate::draw::spatial::Orientation;
use crate::native::test_harness::FakeClipboard;
use crate::native::traits::input::IClipboard;
use crate::tests::common::*;
use crate::ui::foundation::clipboard;
use crate::ui::state::State;
use crate::ui::widgets::input::input::*;

fn render_input_geometry(input: &Input, frame: Rect, surface_size: (i32, i32)) -> Vec<u32> {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../../assets/fonts/lucide.ttf"))
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
        surface_size.0,
        surface_size.1,
    );
    WidgetRender::render(input, frame, &mut ctx, &tree);
    canvas.surface().pixels().to_vec()
}

#[test]
fn sync_from_preserves_runtime_value() {
    let mut input = Input::new("old").with_value("kept");
    input.cursor_char = 2;
    input.sync_from(Input::new("new"));
    assert_eq!(input.current_value(), "kept");
    assert_eq!(input.placeholder, "new");
    assert_eq!(input.cursor_char, 2);
}

#[test]
fn sync_from_preserves_password_visibility_while_password_mode_remains_active() {
    let mut input = Input::password().with_value("秘密");
    render_input_geometry(&input, Rect::new(240.0, 160.0, 180.0, 32.0), (440, 200));
    input.on_event(&SystemEvent::PointerDown {
        pos: Point::new(168.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    input.sync_from(Input::password().placeholder("新提示"));
    assert!(matches!(
        input.snapshot_fields(),
        SnapshotFields::Input {
            password_visible: true,
            ..
        }
    ));

    input.sync_from(Input::new("普通输入"));
    assert!(matches!(
        input.snapshot_fields(),
        SnapshotFields::Input {
            password: false,
            password_visible: false,
            ..
        }
    ));
}

#[test]
fn controlled_value_reads_external_state_and_writes_edits_back() {
    let value = State::new("before".to_string());
    let mut input = Input::new("Name").value(&value);
    assert_eq!(input.current_value(), "before");

    assert_eq!(
        input.on_event(&SystemEvent::TextInput {
            text: "!".to_string(),
        }),
        EventResult::Handled
    );
    assert_eq!(input.current_value(), "before!");
    assert_eq!(value.get(), "before!");

    value.set("outside".to_string());
    input.sync_from(Input::new("Name").value(&value));
    assert_eq!(input.current_value(), "outside");

    assert_eq!(
        input.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(value.get(), "");
}

#[test]
fn documented_input_factories_and_unicode_max_length_are_enforced() {
    let password = Input::password().placeholder("Secret");
    assert!(matches!(
        password.snapshot_fields(),
        SnapshotFields::Input {
            password: true,
            ref placeholder,
            ..
        } if placeholder == "Secret"
    ));

    let mut textarea = Input::textarea().rows(2).max_length(3);
    assert_eq!(
        textarea.on_event(&SystemEvent::TextInput {
            text: "中a🙂x".to_string(),
        }),
        EventResult::Handled
    );
    assert_eq!(textarea.current_value(), "中a🙂");
    assert_eq!(
        textarea.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::SHIFT,
        }),
        EventResult::NotHandled
    );
    assert!(matches!(
        textarea.snapshot_fields(),
        SnapshotFields::Input {
            textarea: true,
            textarea_rows: 2,
            max_length: Some(3),
            ..
        }
    ));
}

#[test]
fn ctrl_v_pastes_from_injected_clipboard() {
    let mut clipboard = FakeClipboard::new();
    clipboard.set_text("clip");

    let mut input = Input::new("").with_value("ab");
    input.cursor_char = 1;
    let result = clipboard::with_clipboard(&mut clipboard, || {
        input.on_event(&SystemEvent::KeyDown {
            key: KeyCode::V,
            mods: KeyMod::CTRL,
        })
    });

    assert_eq!(result, EventResult::Handled);
    assert_eq!(input.current_value(), "aclipb");
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
    assert_eq!(input.current_value(), "aXYd");
}

#[test]
fn textarea_preserves_trailing_empty_line_for_measurement_and_caret() {
    let mut input = Input::textarea().rows(1).with_value("甲\n");
    input.on_event(&SystemEvent::FocusIn);

    let measured = input.measure(Constraints::loose(Size::new(200.0, 200.0)));
    assert_eq!(
        measured.h, 60.0,
        "trailing newline must reserve a second row"
    );

    render_input_geometry(&input, Rect::new(0.0, 0.0, 160.0, 60.0), (160, 64));
    assert!(
        input.caret_rect.get().y >= 28.0,
        "caret after a trailing newline must render on the empty second row: {:?}",
        input.caret_rect.get()
    );
}

#[test]
fn textarea_normalizes_windows_newlines_before_editing() {
    let mut input = Input::textarea();

    assert_eq!(
        input.on_event(&SystemEvent::Paste {
            text: "甲\r\n乙\r丙".to_string(),
        }),
        EventResult::Handled
    );
    assert_eq!(input.current_value(), "甲\n乙\n丙");
    assert_eq!(input.cursor_char, 5);
}

#[test]
fn textarea_scrolled_pointer_hit_uses_the_visible_logical_line() {
    let mut input = Input::textarea().rows(2).with_value("aa\nbb\ncc\ndd");
    input.on_event(&SystemEvent::FocusIn);
    render_input_geometry(&input, Rect::new(0.0, 0.0, 140.0, 56.0), (140, 60));
    assert_eq!(
        input.on_event(&SystemEvent::PointerDown {
            pos: Point::new(100.0, 10.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        input.cursor_char, 8,
        "clicking the right side of visible line `cc` must land at its end"
    );
}

#[test]
fn clicking_an_empty_placeholder_keeps_the_cursor_in_value_bounds() {
    let mut input = Input::new("请输入内容");
    render_input_geometry(&input, Rect::new(0.0, 0.0, 160.0, 32.0), (160, 40));

    input.on_event(&SystemEvent::PointerDown {
        pos: Point::new(100.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(input.cursor_char, 0);

    input.on_event(&SystemEvent::TextInput {
        text: "中".to_string(),
    });
    assert_eq!(input.current_value(), "中");
    assert_eq!(input.cursor_char, 1);
}

#[test]
fn clearable_input_clears_from_its_rendered_action_slot() {
    let mut input = Input::new("").with_value("待清除").clearable(true);
    input.on_event(&SystemEvent::FocusIn);
    render_input_geometry(&input, Rect::new(240.0, 160.0, 180.0, 32.0), (440, 200));

    input.on_event(&SystemEvent::PointerDown {
        pos: Point::new(170.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(input.current_value(), "");
    assert_eq!(input.cursor_char, 0);
}

#[test]
fn password_visibility_action_uses_local_coordinates_after_translation() {
    let mut input = Input::password().with_value("秘密");
    render_input_geometry(&input, Rect::new(240.0, 160.0, 180.0, 32.0), (440, 200));

    input.on_event(&SystemEvent::PointerDown {
        pos: Point::new(168.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert!(matches!(
        input.snapshot_fields(),
        SnapshotFields::Input {
            password_visible: true,
            ..
        }
    ));
}

#[test]
fn cjk_addon_uses_visible_text_width_instead_of_utf8_byte_width() {
    let baseline =
        render_input_geometry(&Input::new(""), Rect::new(0.0, 0.0, 180.0, 32.0), (180, 40));
    let with_addon = render_input_geometry(
        &Input::new("").addon_before("中文"),
        Rect::new(0.0, 0.0, 180.0, 32.0),
        (180, 40),
    );

    let sample = 16 * 180 + 50;
    assert_eq!(
        with_addon[sample], baseline[sample],
        "x=50 must already belong to the input body for a two-character CJK addon"
    );
}

#[test]
fn hidden_password_caret_tracks_the_masked_prefix_and_inline_preedit() {
    const GLYPH: &str = "\u{E151}";
    let mut password = Input::password().with_value(GLYPH.repeat(4));
    password.cursor_char = 2;
    password.composition = "A".to_string();
    password.on_event(&SystemEvent::FocusIn);
    render_input_geometry(&password, Rect::new(0.0, 0.0, 220.0, 32.0), (220, 40));

    let mut visual_equivalent = Input::new("").with_value("\u{2022}\u{2022}A\u{2022}\u{2022}");
    visual_equivalent.cursor_char = 3;
    visual_equivalent.on_event(&SystemEvent::FocusIn);
    render_input_geometry(
        &visual_equivalent,
        Rect::new(0.0, 0.0, 220.0, 32.0),
        (220, 40),
    );

    assert!(
        (password.caret_rect.get().x - visual_equivalent.caret_rect.get().x).abs() < 0.01,
        "masked password caret must follow two bullets plus inline preedit: password={:?}, expected={:?}",
        password.caret_rect.get(),
        visual_equivalent.caret_rect.get()
    );
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
    assert_eq!(input.current_value(), "");

    assert_eq!(
        input.on_event(&SystemEvent::ImeCompositionEnd {
            text: "中".to_string(),
        }),
        EventResult::Handled
    );
    assert!(input.composition.is_empty());
    assert_eq!(input.current_value(), "");
    assert_eq!(
        input.on_event(&SystemEvent::TextInput {
            text: "中".to_string(),
        }),
        EventResult::Handled
    );
    assert_eq!(input.current_value(), "中");
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
            let mut ctx = PaintContext::new_for_test(
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
                ctx.with_recorder(&mut list, |ctx| {
                    WidgetRender::render(input, frame, ctx, &tree);
                });
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
