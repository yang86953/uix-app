//! 布局引擎与真实布局组件使用同一 border-box 契约。
#![cfg(feature = "test-harness")]
use uix::prelude::*;
use uix::ui::test_harness::TestApp;

fn check(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.01,
        "actual={actual}, expected={expected}"
    );
}

#[test]
fn declared_container_and_grid_sizes_include_their_borders() {
    for grid in [false, true] {
        let app = TestApp::new((300.0, 180.0), move || {
            let mut style = Style::default();
            style.width = Some(100.0);
            style.height = Some(60.0);
            style.padding = EdgeInsets::uniform(5.0);
            style.border_width = EdgeInsets::uniform(2.0);
            let child = if grid {
                Grid::new().style(style).build()
            } else {
                Container::new().style(style).build()
            };
            column((child.automation_id("box"),)).align(AlignItems::Start)
        });
        let frame = app.snapshot().find("box").unwrap().frame;
        check(frame.w, 100.0);
        check(frame.h, 60.0);
    }
}

#[test]
fn empty_auto_boxes_keep_padding_and_border_but_not_margin() {
    for grid in [false, true] {
        let app = TestApp::new((300.0, 180.0), move || {
            let mut style = Style::default();
            style.padding = EdgeInsets::new(7.0, 11.0, 13.0, 17.0);
            style.border_width = EdgeInsets::new(2.0, 3.0, 4.0, 5.0);
            style.margin = EdgeInsets::new(19.0, 23.0, 29.0, 31.0);
            let child = if grid {
                Grid::new().style(style).build()
            } else {
                Container::new().style(style).build()
            };
            column((child.automation_id("box"),)).align(AlignItems::Start)
        });
        let frame = app.snapshot().find("box").unwrap().frame;
        check(frame.x, 19.0);
        check(frame.y, 23.0);
        check(frame.w, 26.0);
        check(frame.h, 36.0);
    }
}

#[test]
fn asymmetric_box_layers_are_consumed_exactly_once() {
    let app = TestApp::new((300.0, 180.0), || {
        let mut style = Style::default();
        style.width = Some(100.0);
        style.height = Some(60.0);
        style.padding = EdgeInsets::new(7.0, 11.0, 13.0, 17.0);
        style.border_width = EdgeInsets::new(2.0, 3.0, 4.0, 5.0);
        style.margin = EdgeInsets::new(19.0, 23.0, 29.0, 31.0);
        row((ViewNode::new(
            Container::new().style(style),
            vec![
                Container::new()
                    .flex_grow(1.0)
                    .build()
                    .automation_id("content"),
            ],
        )
        .automation_id("box"),))
        .align(AlignItems::Center)
    });
    let snapshot = app.snapshot();
    let frame = snapshot.find("box").unwrap().frame;
    let content = snapshot.find("content").unwrap().frame;
    check(frame.x, 19.0);
    check(frame.y, 56.0);
    check(frame.w, 100.0);
    check(frame.h, 60.0);
    check(content.x, 28.0);
    check(content.y, 70.0);
    check(content.w, 74.0);
    check(content.h, 24.0);
}

#[test]
fn grow_grid_retains_natural_cross_axis_height() {
    let app = TestApp::new((300.0, 180.0), || {
        row((ViewNode::new(
            Grid::new().columns(vec![GridTrack::Auto]),
            vec![Container::new().size(80.0, 20.0).build()],
        )
        .flex_grow(1.0)
        .automation_id("grid"),))
        .align(AlignItems::Center)
    });
    let frame = app.snapshot().find("grid").unwrap().frame;
    check(frame.w, 300.0);
    check(frame.h, 20.0);
    check(frame.y, 80.0);
}

#[test]
fn flex_shrink_cannot_erase_box_insets() {
    let app = TestApp::new((20.0, 100.0), || {
        row((
            Container::new()
                .padding(EdgeInsets::uniform(12.0))
                .border(Color::BLACK, 2.0)
                .build()
                .automation_id("first"),
            Container::new()
                .padding(EdgeInsets::uniform(12.0))
                .border(Color::BLACK, 2.0)
                .build()
                .automation_id("second"),
        ))
        .align(AlignItems::Center)
    });
    let snapshot = app.snapshot();
    for id in ["first", "second"] {
        let frame = snapshot.find(id).unwrap().frame;
        check(frame.w, 28.0);
        check(frame.h, 28.0);
    }
}

#[test]
fn engine_intrinsic_cross_size_does_not_compress_content_in_overflow_mode() {
    let child = LayoutChild::new(WidgetId::new(1), Size::new(20.0, 80.0));
    let mut engine = FlexLayout::row();
    engine.intrinsic_cross = true;
    engine.align = AlignItems::Stretch;
    for wrap in [false, true] {
        engine.wrap = wrap;
        for overflow in [false, true] {
            engine.overflow_content = overflow;
            let output = engine.layout(
                Rect::new(0.0, 0.0, 100.0, 10.0),
                std::slice::from_ref(&child),
            );
            check(output.positions[0].h, 80.0);
        }
    }
}

#[test]
fn empty_intrinsic_engine_has_no_content_extent() {
    let mut engine = FlexLayout::row();
    engine.intrinsic_main = true;
    engine.intrinsic_cross = true;
    for direction in [FlexDirection::Row, FlexDirection::Column] {
        engine.direction = direction;
        assert_eq!(
            engine
                .layout(Rect::new(0.0, 0.0, 100.0, 80.0), &[])
                .total_size,
            Size::zero()
        );
    }
}

#[test]
fn grid_tracks_cannot_erase_child_box_insets() {
    let app = TestApp::new((20.0, 100.0), || {
        ViewNode::new(
            Grid::new()
                .columns(vec![GridTrack::Px(10.0)])
                .rows(vec![GridTrack::Px(10.0)]),
            vec![
                Container::new()
                    .padding(EdgeInsets::uniform(12.0))
                    .border(Color::BLACK, 2.0)
                    .build()
                    .automation_id("box"),
            ],
        )
    });
    let frame = app.snapshot().find("box").unwrap().frame;
    check(frame.w, 28.0);
    check(frame.h, 28.0);
}
