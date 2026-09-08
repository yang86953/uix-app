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

#[test]
fn static_and_dynamic_text_obey_the_same_nested_width_constraints() {
    let width = State::new(210.0_f32);
    let root_width = width.clone();
    let mut app = TestApp::new((400.0, 700.0), move || {
        let content =
            "从代码事实生成模块、组件与依赖；先预览，再由人确认合入。任务进度与验收在任务页管理。"
                .to_owned();
        column((
            column((
                uix!(r#"<Text automationId="static" fontSize="12">从代码事实生成模块、组件与依赖；先预览，再由人确认合入。任务进度与验收在任务页管理。</Text>"#),
                uix!(r#"<Text automationId="dynamic" fontSize="12">{content}</Text>"#),
                label("后继节点").automation_id("next"),
            )).gap(8.0).padding(12.0).width(root_width.get()),
        )).width(350.0).align(AlignItems::Start)
    });
    for available in [210.0, 120.0, 300.0] {
        width.set(available);
        app.settle().unwrap();
        let snapshot = app.snapshot();
        let static_text = snapshot.find("static").unwrap().frame;
        let dynamic_text = snapshot.find("dynamic").unwrap().frame;
        let next = snapshot.find("next").unwrap().frame;
        assert!(dynamic_text.h > 18.0);
        assert_eq!(
            static_text.h, dynamic_text.h,
            "literal Text must not bypass wrapping"
        );
        assert!(
            static_text.w <= available - 24.0 && dynamic_text.w <= available - 24.0,
            "available={available} static={static_text:?} dynamic={dynamic_text:?}"
        );
        assert!(dynamic_text.y >= static_text.y + static_text.h);
        assert!(next.y >= dynamic_text.y + dynamic_text.h);
    }
}

#[test]
fn text_height_reflows_after_row_siblings_take_width() {
    let mut app = TestApp::new((400.0, 700.0), || {
        let content = "从代码事实生成模块、组件与依赖；先预览，再由人确认合入。".to_owned();
        column((
            row((
                label("icon").width(80.0).flex_shrink(0.0),
                uix!(r#"<Text automationId="row-static" fontSize="12">从代码事实生成模块、组件与依赖；先预览，再由人确认合入。</Text>"#).flex_shrink(1.0),
            )).gap(8.0).width(208.0),
            row((
                label("icon").width(80.0).flex_shrink(0.0),
                uix!(r#"<Text automationId="row-dynamic" fontSize="12">{content}</Text>"#).flex_shrink(1.0),
            )).gap(8.0).width(208.0),
            uix!(r#"<Text automationId="reference" fontSize="12" width="120">从代码事实生成模块、组件与依赖；先预览，再由人确认合入。</Text>"#),
            label("next").automation_id("next"),
        )).width(300.0).gap(8.0).align(AlignItems::Start)
    });
    app.settle().unwrap();
    let snapshot = app.snapshot();
    let reference = snapshot.find("reference").unwrap().frame;
    for id in ["row-static", "row-dynamic"] {
        let frame = snapshot.find(id).unwrap().frame;
        assert_eq!(frame.w, 120.0, "{id}: {frame:?}, reference={reference:?}");
        assert_eq!(
            frame.h, reference.h,
            "{id}: final allocated width must determine height"
        );
    }
}

#[test]
fn inherited_column_width_and_reactive_wrap_policy_remeasure() {
    let wrap = State::new(false);
    let root_wrap = wrap.clone();
    let mut app = TestApp::new((180.0, 600.0), move || {
        column((
            column((
                Label::new("从代码事实生成模块、组件与依赖；先预览，再由人确认合入。")
                    .font_size(12.0).word_wrap(root_wrap.get()).build().automation_id("policy"),
                uix!(r#"<Text automationId="inherited" fontSize="12">从代码事实生成模块、组件与依赖；先预览，再由人确认合入。</Text>"#),
            )).padding(10.0),
        ))
    });
    let before = app.snapshot();
    assert_eq!(before.find("policy").unwrap().frame.h, 18.0);
    assert!(before.find("inherited").unwrap().frame.h > 18.0);
    wrap.set(true);
    app.settle().unwrap();
    let after = app.snapshot();
    assert_eq!(
        after.find("policy").unwrap().frame.h,
        after.find("inherited").unwrap().frame.h
    );
    wrap.set(false);
    app.settle().unwrap();
    assert_eq!(app.snapshot().find("policy").unwrap().frame.h, 18.0);
}
