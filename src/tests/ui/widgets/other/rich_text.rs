use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::{RichText, RichTextSegment, RichTextStyle};
use crate::ui::{AccessibilityRole, ComponentId};

fn render_rich_text(text: &RichText, frame: Rect) -> String {
    render_rich_text_with_tokens(text, frame, DesignTokens::antd_light())
}

fn render_rich_text_with_tokens(text: &RichText, frame: Rect, tokens: DesignTokens) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(320, 160));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tree = WidgetTree::new();
    let mut display_list = crate::draw::painting::DisplayList::new();
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
            320,
            160,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(text, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

#[test]
fn rich_text_default_color_tracks_theme_and_explicit_color_stays_stable() {
    let default = RichText::new().content(vec![RichTextSegment::Text {
        content: "theme contrast".to_string(),
        style: RichTextStyle::default(),
    }]);
    let light = render_rich_text_with_tokens(
        &default,
        Rect::new(0.0, 0.0, 120.0, 24.0),
        DesignTokens::antd_light(),
    );
    let dark = render_rich_text_with_tokens(
        &default,
        Rect::new(0.0, 0.0, 120.0, 24.0),
        DesignTokens::antd_dark(),
    );
    assert_ne!(
        light, dark,
        "default RichText must resolve the theme text color"
    );

    let explicit = RichText::new()
        .content(vec![RichTextSegment::Text {
            content: "fixed color".to_string(),
            style: RichTextStyle::default(),
        }])
        .color(Color::from_rgb(12, 34, 56));
    let light = render_rich_text_with_tokens(
        &explicit,
        Rect::new(0.0, 0.0, 120.0, 24.0),
        DesignTokens::antd_light(),
    );
    let dark = render_rich_text_with_tokens(
        &explicit,
        Rect::new(0.0, 0.0, 120.0, 24.0),
        DesignTokens::antd_dark(),
    );
    assert_eq!(
        light, dark,
        "explicit RichText color must override the theme"
    );
}

#[test]
fn rich_text_code_copy_region_uses_node_local_coordinates() {
    let mut text = RichText::new().content(vec![RichTextSegment::Code {
        content: "code".to_string(),
    }]);
    let _ = render_rich_text(&text, Rect::new(80.0, 40.0, 200.0, 40.0));
    let copy = text.code_copy_rect_for_test(0).expect("copy button");
    assert!(
        copy.x < 80.0 && copy.y < 40.0,
        "copy region must be local: {copy:?}"
    );

    let down = SystemEvent::PointerDown {
        pos: Point::new(copy.x + copy.w * 0.5, copy.y + copy.h * 0.5),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(text.on_event(&down), EventResult::Handled);
    assert_eq!(text.take_pending_copy(), None);

    let up = SystemEvent::PointerUp {
        pos: Point::new(copy.x + copy.w * 0.5, copy.y + copy.h * 0.5),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(text.on_event(&up), EventResult::Handled);

    assert_eq!(text.take_pending_copy().as_deref(), Some("code"));

    assert_eq!(text.on_event(&down), EventResult::Handled);
    assert_eq!(
        text.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(text.on_event(&up), EventResult::NotHandled);
    assert_eq!(text.take_pending_copy(), None);
}

#[test]
fn rich_text_draws_the_actual_wrapped_line_runs() {
    let text = RichText::new().content(vec![RichTextSegment::Text {
        content: "alpha beta gamma".to_string(),
        style: RichTextStyle::default(),
    }]);

    let display_list = render_rich_text(&text, Rect::new(10.0, 20.0, 55.0, 37.0));
    let lines = text.layout_line_texts_for_test();
    assert!(lines.len() > 1, "narrow frame should wrap: {lines:?}");
    assert_eq!(lines.concat(), "alpha beta gamma");
    assert!(lines.iter().all(|line| line != "alpha beta gamma"));
    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 20.0, w: 55.0, h: 37.0 } }"),
        "{display_list}"
    );
}

#[test]
fn rich_text_pointer_and_keyboard_links_submit_urls() {
    let mut text = RichText::new().content(vec![
        RichTextSegment::Link {
            content: "Guide".to_string(),
            url: "https://example.test/guide".to_string(),
        },
        RichTextSegment::Text {
            content: " or ".to_string(),
            style: RichTextStyle::default(),
        },
        RichTextSegment::Link {
            content: "API".to_string(),
            url: "https://example.test/api".to_string(),
        },
    ]);
    let _ = render_rich_text(&text, Rect::new(80.0, 40.0, 200.0, 40.0));

    let point = text.link_point_for_test(0).expect("first link point");
    let click = SystemEvent::PointerDown {
        pos: point,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(text.on_event(&click), EventResult::Handled);
    let pressed = render_rich_text(&text, Rect::new(80.0, 40.0, 200.0, 40.0));
    assert!(text.semantic_event(ComponentId::new(9), &click).is_none());
    let release = SystemEvent::PointerUp {
        pos: point,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(text.on_event(&release), EventResult::Handled);
    let released = render_rich_text(&text, Rect::new(80.0, 40.0, 200.0, 40.0));
    assert_ne!(
        pressed, released,
        "released link must clear pressed feedback"
    );
    assert_eq!(
        text.semantic_event(ComponentId::new(9), &release)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("https://example.test/guide".to_string())
    );

    assert_eq!(text.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(
        text.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let submit = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };
    assert_eq!(text.on_event(&submit), EventResult::Handled);
    assert_eq!(
        text.semantic_event(ComponentId::new(9), &submit)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("https://example.test/api".to_string())
    );

    assert_eq!(
        text.focused_link(),
        Some(("API", "https://example.test/api"))
    );
    let accessibility = text.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.name.as_deref(), Some("API"));
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("https://example.test/api")
    );
}

#[test]
fn rich_text_rejects_outside_pointer_actions_and_normalizes_invalid_frames() {
    let mut text = RichText::new().content(vec![RichTextSegment::Link {
        content: "Guide".to_string(),
        url: "https://example.test/guide".to_string(),
    }]);
    let _ = render_rich_text(&text, Rect::new(10.0, 20.0, 80.0, 22.0));

    assert_eq!(
        text.on_event(&SystemEvent::PointerDown {
            pos: Point::new(81.0, 10.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );

    let invalid = render_rich_text(&text, Rect::new(0.0, 0.0, f32::NAN, -1.0));
    assert_eq!(invalid, "DisplayList { ops: [] }");
}
