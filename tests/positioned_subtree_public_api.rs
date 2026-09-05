#![cfg(feature = "test-harness")]

//! 对应 UIX Lang 样式属性中的 absolute/fixed 定位与包含块契约。

use uix::prelude::*;
use uix::ui::PositionMode;
use uix::ui::test_harness::TestApp;

#[test]
fn absolute_container_keeps_declared_size_and_clickable_descendants() {
    let clicked = State::new(false);
    let view_clicked = clicked.clone();
    let mut app = TestApp::new((600.0, 400.0), move || {
        let clicked = view_clicked.clone();
        column((column((button("节点")
            .on_click_fn(move || clicked.set(true))
            .build()
            .automation_id("node"),))
        .width(220.0)
        .height(72.0)
        .position(PositionMode::Absolute)
        .left(24.0)
        .top(40.0)
        .automation_id("card"),))
        .width(500.0)
        .height(300.0)
        .position(PositionMode::Relative)
    });
    let snapshot = app.snapshot();
    let card = snapshot.find("card").unwrap();
    assert_eq!(card.frame, Rect::new(24.0, 40.0, 220.0, 72.0));
    assert!(snapshot.find("node").unwrap().visible_bounds.is_some());
    app.click("node").expect("绝对定位卡片中的按钮应可命中");
    assert!(clicked.get());
}

#[test]
fn absolute_auto_size_includes_nested_content() {
    let app = TestApp::new((600.0, 400.0), || {
        column((column((label("中文结构节点").automation_id("label"),))
            .position(PositionMode::Absolute)
            .left(24.0)
            .top(40.0)
            .map_style(|style| style.clip_content = Some(true))
            .automation_id("card"),))
        .width(500.0)
        .height(300.0)
        .position(PositionMode::Relative)
    });
    let snapshot = app.snapshot();
    let card = snapshot.find("card").unwrap();
    assert!(card.frame.w > 0.0 && card.frame.h > 0.0, "{card:?}");
    assert!(snapshot.find("label").unwrap().visible_bounds.is_some());
}

#[test]
fn fixed_auto_size_tracks_nested_content_changes() {
    let text = State::new("初始节点".to_owned());
    let view_text = text.clone();
    let mut app = TestApp::new((600.0, 400.0), move || {
        let text = view_text.clone();
        column((column((column_fit((
            dynamic_label(move || text.get()).automation_id("label"),
        )),))
        .position(PositionMode::Fixed)
        .right(24.0)
        .top(40.0)
        .automation_id("card"),))
    });
    let before = app.snapshot().find("card").unwrap().frame;
    assert!(before.w > 0.0 && before.h > 0.0);
    assert!((before.x + before.w - 576.0).abs() < 0.1);
    text.set("更新节点\n第二行".to_owned());
    app.settle().unwrap();
    let after = app.snapshot().find("card").unwrap().frame;
    assert!(
        after.h > before.h,
        "before={before:?}, after={after:?}, snapshot={}",
        app.snapshot().to_json()
    );
    assert!(
        app.snapshot()
            .find("label")
            .unwrap()
            .visible_bounds
            .is_some()
    );
}

#[test]
fn opposite_insets_keep_stretching_after_resize() {
    let mut app = TestApp::new((600.0, 400.0), || {
        column((column((label("节点"),))
            .position(PositionMode::Absolute)
            .left(24.0)
            .right(24.0)
            .top(40.0)
            .bottom(24.0)
            .automation_id("card"),))
        .position(PositionMode::Relative)
    });
    assert_eq!(
        app.snapshot().find("card").unwrap().frame,
        Rect::new(24.0, 40.0, 552.0, 336.0)
    );
    app.resize(800.0, 500.0).unwrap();
    assert_eq!(
        app.snapshot().find("card").unwrap().frame,
        Rect::new(24.0, 40.0, 752.0, 436.0)
    );
}
