use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::{RichText, RichTextSegment, RichTextStyle};
use crate::ui::{AccessibilityRole, ComponentId};

fn render_rich_text(text: &RichText, frame: Rect) {
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
    WidgetRender::render(text, frame, &mut ctx, &tree);
}

#[test]
fn rich_text_code_copy_region_uses_node_local_coordinates() {
    let mut text = RichText::new().content(vec![RichTextSegment::Code {
        content: "code".to_string(),
    }]);
    render_rich_text(&text, Rect::new(80.0, 40.0, 200.0, 40.0));
    let copy = text.code_copy_rect_for_test(0).expect("copy button");
    assert!(
        copy.x < 80.0 && copy.y < 40.0,
        "copy region must be local: {copy:?}"
    );

    let _ = text.on_event(&SystemEvent::PointerDown {
        pos: Point::new(copy.x + copy.w * 0.5, copy.y + copy.h * 0.5),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(text.take_pending_copy().as_deref(), Some("code"));
}

#[test]
fn rich_text_draws_the_actual_wrapped_line_runs() {
    let text = RichText::new().content(vec![RichTextSegment::Text {
        content: "alpha beta gamma".to_string(),
        style: RichTextStyle::default(),
    }]);

    render_rich_text(&text, Rect::new(10.0, 20.0, 55.0, 120.0));
    let lines = text.layout_line_texts_for_test();
    assert!(lines.len() > 1, "narrow frame should wrap: {lines:?}");
    assert_eq!(lines.concat(), "alpha beta gamma");
    assert!(lines.iter().all(|line| line != "alpha beta gamma"));
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
    render_rich_text(&text, Rect::new(80.0, 40.0, 200.0, 40.0));

    let point = text.link_point_for_test(0).expect("first link point");
    let click = SystemEvent::PointerDown {
        pos: point,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(text.on_event(&click), EventResult::Handled);
    assert_eq!(
        text.semantic_event(ComponentId::new(9), &click)
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
