#![cfg(feature = "test-harness")]

use uix::prelude::*;
use uix::ui::test_harness::TestApp;

#[test]
fn dynamic_text_remeasures_declared_line_height_after_content_changes() {
    let text = State::new("第一行".to_owned());
    let root_text = text.clone();
    let mut app = TestApp::new((320.0, 300.0), move || {
        let message = root_text.clone();
        column((
            uix!(r#"<Text automationId="text" fontSize="14">{message.get()}</Text>"#)
                .map_style(|style| style.line_height = LineHeight::pixels(40.0)),
        ))
        .width(280.0)
    });
    assert_eq!(app.snapshot().find("text").unwrap().frame.h, 40.0);
    text.set("第一行\n第二行".to_owned());
    app.settle().unwrap();
    assert_eq!(app.snapshot().find("text").unwrap().frame.h, 80.0);
}

#[test]
fn dynamic_text_measures_the_current_subtree_theme() {
    let font_size = State::new(28.0_f32);
    let root_size = font_size.clone();
    let mut app = TestApp::new((400.0, 320.0), move || {
        let mut tokens = DesignTokens::antd_light();
        tokens.font_size = root_size.get();
        let config = WidgetConfig::new().theme(Theme::new(tokens));
        with_config(&config, || {
            column((dynamic_label(|| "主题正文".to_owned()).automation_id("text"),)).width(300.0)
        })
    });
    assert_eq!(app.snapshot().find("text").unwrap().frame.h, 42.0);
    font_size.set(20.0);
    app.settle().unwrap();
    assert_eq!(app.snapshot().find("text").unwrap().frame.h, 30.0);
}
