//! Typography uses one em/line-box contract across public declarative controls.
#![cfg(all(feature = "test-harness", feature = "rich-text"))]
use uix::prelude::*;
use uix::ui::test_harness::TestApp;

fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.05, "{actual} != {expected}");
}

#[test]
fn static_and_dynamic_text_share_the_same_normal_line_box() {
    let app = TestApp::new((600.0, 100.0), || {
        let message = State::new("中文 Hg".to_owned());
        uix!(
            r#"<Container direction="row" align="center" height="60px"><Text fontSize="24px" automationId="static">中文 Hg</Text><Text fontSize="24px" automationId="dynamic">{message.get()}</Text></Container>"#
        )
    });
    let snapshot = app.snapshot();
    let a = snapshot.find("static").unwrap().frame;
    let b = snapshot.find("dynamic").unwrap().frame;
    near(a.h, 36.0);
    near(a.y, b.y);
    near(a.h, b.h);
}

#[test]
fn input_line_height_drives_intrinsic_height_and_reconciles() {
    let large = State::new(false);
    let build_large = large.clone();
    let mut app = TestApp::new((400.0, 300.0), move || {
        let size = if build_large.get() { 32.0 } else { 14.0 };
        column((input()
            .placeholder("中文 Hg")
            .automation_id("input")
            .map_style(move |s| {
                s.font_size = TypographyToken::Custom(size);
                s.line_height = LineHeight::factor(1.5);
            }),))
    });
    let before = app.snapshot().find("input").unwrap().frame;
    large.set(true);
    app.settle().unwrap();
    let after = app.snapshot().find("input").unwrap().frame;
    assert!(
        after.h >= 48.0 && after.h > before.h,
        "{before:?} -> {after:?}"
    );
}

#[test]
fn rich_text_declarative_font_size_width_and_line_height_reach_measurement() {
    let app = TestApp::new((600.0, 200.0), || {
        column((uix!(
            r#"<RichText content="中文 Hg" fontSize="32px" width="240px" automationId="rich" />"#
        )
        .map_style(|s| s.line_height = LineHeight::factor(2.0))
        .flex_grow(0.0),))
        .map_style(|s| s.align_items = AlignItems::Start)
    });
    let frame = app.snapshot().find("rich").unwrap().frame;
    near(frame.w, 240.0);
    near(frame.h, 64.0);
}

#[test]
fn default_text_controls_share_the_current_theme_font_size() {
    let font_size = State::new(24.0);
    let root_size = font_size.clone();
    let mut app = TestApp::new((800.0, 400.0), move || {
        let mut tokens = DesignTokens::antd_light();
        tokens.font_size = root_size.get();
        let config = WidgetConfig::new().theme(Theme::new(tokens));
        with_config(&config, || {
            column_fit((
                label("中文 Hg").automation_id("static"),
                dynamic_label(|| "中文 Hg".to_owned()).automation_id("dynamic"),
                RichText::new()
                    .content(parse_rich_text("中文 Hg"))
                    .build()
                    .flex_grow(0.0)
                    .automation_id("rich"),
                input().placeholder("中文 Hg").automation_id("input"),
                label("Explicit")
                    .font_size(14.0)
                    .build()
                    .width(200.0)
                    .automation_id("explicit"),
            ))
            .map_style(|style| style.align_items = AlignItems::Start)
        })
    });
    for size in [24.0, 18.0] {
        font_size.set(size);
        app.settle().unwrap();
        let snapshot = app.snapshot();
        for id in ["static", "dynamic", "rich"] {
            near(snapshot.find(id).unwrap().frame.h, size * 1.5);
        }
        near(snapshot.find("explicit").unwrap().frame.h, 21.0);
        assert!(snapshot.find("input").unwrap().frame.h >= size * 1.5);
    }
}

#[test]
fn typography_reconcile_preserves_input_focus_selection_and_value() {
    let large = State::new(false);
    let build_large = large.clone();
    let value = State::new("前中文🙂后".to_owned());
    let build_value = value.clone();
    let mut app = TestApp::new((600.0, 240.0), move || {
        let size = if build_large.get() { 32.0 } else { 14.0 };
        column_fit((input()
            .value(&build_value)
            .automation_id("input")
            .map_style(move |style| {
                style.font_size = TypographyToken::Custom(size);
                style.line_height = LineHeight::factor(1.65);
            }),))
    });
    app.focus("input").unwrap();
    app.press_key(KeyCode::Home, KeyMod::NONE).unwrap();
    app.press_key(KeyCode::Right, KeyMod::NONE).unwrap();
    for _ in 0..3 {
        app.press_key(KeyCode::Right, KeyMod::SHIFT).unwrap();
    }
    large.set(true);
    app.settle().unwrap();
    assert_eq!(value.get(), "前中文🙂后");
    app.insert_text("input", "替换").unwrap();
    assert_eq!(value.get(), "前替换后");
}

#[test]
fn rich_text_style_changes_invalidate_measured_line_boxes() {
    let large = State::new(false);
    let build_large = large.clone();
    let mut app = TestApp::new((600.0, 300.0), move || {
        let size = if build_large.get() { 32.0 } else { 14.0 };
        column_fit((RichText::new()
            .content(parse_rich_text("中文\n第二行"))
            .build()
            .flex_grow(0.0)
            .automation_id("rich")
            .map_style(move |style| {
                style.font_size = TypographyToken::Custom(size);
                style.line_height = LineHeight::factor(2.0);
            }),))
    });
    near(app.snapshot().find("rich").unwrap().frame.h, 56.0);
    large.set(true);
    app.settle().unwrap();
    near(app.snapshot().find("rich").unwrap().frame.h, 128.0);
}

#[test]
fn typography_reconcile_keeps_ime_preedit_uncommitted_until_text_input() {
    let large = State::new(false);
    let build_large = large.clone();
    let value = State::new("前后".to_owned());
    let build_value = value.clone();
    let mut app = TestApp::new((600.0, 240.0), move || {
        let size = if build_large.get() { 32.0 } else { 14.0 };
        column_fit((input()
            .value(&build_value)
            .automation_id("input")
            .map_style(move |s| {
                s.font_size = TypographyToken::Custom(size);
            }),))
    });
    app.focus("input").unwrap();
    app.press_key(KeyCode::Home, KeyMod::NONE).unwrap();
    app.press_key(KeyCode::Right, KeyMod::NONE).unwrap();
    app.dispatch_system_event(&SystemEvent::ImeCompositionStart)
        .unwrap();
    app.dispatch_system_event(&SystemEvent::ImeCompositionUpdate {
        text: "中文".to_owned(),
    })
    .unwrap();
    large.set(true);
    app.settle().unwrap();
    assert_eq!(value.get(), "前后");
    app.dispatch_system_event(&SystemEvent::ImeCompositionEnd {
        text: "中文".to_owned(),
    })
    .unwrap();
    app.dispatch_system_event(&SystemEvent::TextInput {
        text: "中文".to_owned(),
    })
    .unwrap();
    assert_eq!(value.get(), "前中文后");
}

#[test]
fn textarea_intrinsic_height_uses_declared_padding_and_line_height() {
    let app = TestApp::new((600.0, 400.0), || {
        column_fit((uix!(
            r#"<Input type="textarea" fontSize="32px" padding="16px" automationId="area" />"#
        )
        .map_style(|s| s.line_height = LineHeight::factor(2.0)),))
    });
    // Three default rows, each 64px, plus both 16px vertical insets.
    near(app.snapshot().find("area").unwrap().frame.h, 224.0);
}
