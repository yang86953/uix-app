//! Flex basis 只影响父主轴，不能清空交叉轴的自然内容尺寸。
#![cfg(feature = "test-harness")]
use uix::prelude::*;
use uix::ui::test_harness::TestApp;

#[test]
fn growing_wrapper_keeps_its_cross_axis_content_height() {
    let mut failures = Vec::new();
    for (grow, height) in [(0.0_f32, 0.0_f32), (1.0, 0.0), (1.0, 21.0)] {
        let mut app = TestApp::new((600.0, 200.0), move || {
            column((uix!(
                r#"<Container direction="row" align="center" height="48px" width="600px" automationId="row">
                <Container width="26px" align="center"><Icon name="circle-check" size="17px" automationId="icon" /></Container>
                <Container flexGrow={grow} height={height} automationId="wrapper">
                    <Container direction="row" gap="5px" align="center" automationId="text-row">
                        <Text fontSize="14px" automationId="title">框架对齐</Text>
                        <Text fontSize="12px" automationId="status">· 进行中</Text>
                    </Container>
                </Container>
            </Container>"#
            ),))
        });
        app.settle().unwrap();
        let snapshot = app.snapshot();
        let row = snapshot.find("row").unwrap().frame;
        let wrapper = snapshot.find("wrapper").unwrap().frame;
        let inner = snapshot.find("text-row").unwrap().frame;
        let title = snapshot.find("title").unwrap().frame;
        let icon = snapshot.find("icon").unwrap().frame;
        let delta = title.y + title.h / 2.0 - (icon.y + icon.h / 2.0);
        println!(
            "grow={grow} explicit_height={height} row={row:?} wrapper={wrapper:?} inner={inner:?} title={title:?} icon={icon:?} delta={delta}"
        );
        if (wrapper.h - 21.0).abs() > 0.01 || delta.abs() > 0.01 {
            failures.push((grow, wrapper.h, delta));
        }
    }
    assert!(
        failures.is_empty(),
        "flex-grow may zero the main-axis basis, not the cross-axis content: {failures:?}"
    );
}

#[test]
fn growing_container_preserves_cross_axis_in_all_parent_directions() {
    for direction in [
        FlexDirection::Row,
        FlexDirection::RowReverse,
        FlexDirection::Column,
        FlexDirection::ColumnReverse,
    ] {
        let is_row = matches!(direction, FlexDirection::Row | FlexDirection::RowReverse);
        let app = TestApp::new((400.0, 160.0), move || {
            ViewNode::new(
                Container::new().dir(direction),
                vec![
                    Container::new()
                        .size(16.0, 16.0)
                        .build()
                        .automation_id("fixed"),
                    ViewNode::new(
                        // 子容器故意保持 Column；不能以它自己的方向裁决父布局的交叉轴。
                        Container::new().flex_grow(1.0),
                        vec![Container::new().size(80.0, 20.0).build()],
                    )
                    .automation_id("growing"),
                ],
            )
            .align(AlignItems::Center)
        });
        let snapshot = app.snapshot();
        let frame = snapshot.find("growing").unwrap().frame;
        let (main, cross, start, expected_main, expected_cross, available_cross) = if is_row {
            (frame.w, frame.h, frame.y, 384.0, 20.0, 160.0)
        } else {
            (frame.h, frame.w, frame.x, 144.0, 80.0, 400.0)
        };
        assert!(
            (main - expected_main).abs() < 0.01,
            "{direction:?}: basis must remain zero: {frame:?}"
        );
        assert!(
            (cross - expected_cross).abs() < 0.01,
            "{direction:?}: natural cross axis: {frame:?}"
        );
        assert!(
            (start + cross / 2.0 - available_cross / 2.0).abs() < 0.01,
            "{direction:?}: center: {frame:?}"
        );
    }
}

#[test]
fn growing_row_wrapper_reflows_and_recenters_after_width_and_text_changes() {
    let width = State::new(340.0_f32);
    let message = State::new("修正框架交叉轴测量，保留真实内容尺寸与自动换行。".to_owned());
    let root_width = width.clone();
    let root_message = message.clone();
    let mut app = TestApp::new((500.0, 700.0), move || {
        let text = root_message.clone();
        column((row((
            Container::new()
                .size(40.0, 17.0)
                .build()
                .automation_id("icon"),
            column((uix!(
                r#"<Text automationId="text" fontSize="14">{text.get()}</Text>"#
            ),))
            .flex_grow(1.0)
            .automation_id("wrapper"),
        ))
        .width(root_width.get())
        .height(300.0)
        .align(AlignItems::Center)
        .automation_id("row"),))
        .align(AlignItems::Start)
    });
    for available in [340.0, 180.0, 440.0, 220.0] {
        width.set(available);
        app.settle().unwrap();
        let snapshot = app.snapshot();
        let wrapper = snapshot.find("wrapper").unwrap().frame;
        let text = snapshot.find("text").unwrap().frame;
        let icon = snapshot.find("icon").unwrap().frame;
        assert!((wrapper.w - (available - 40.0)).abs() < 0.01, "{wrapper:?}");
        assert!(text.h >= 21.0, "text has a natural line height: {text:?}");
        if available <= 340.0 {
            assert!(text.h > 21.0, "text should wrap: {text:?}");
        }
        assert!((wrapper.h - text.h).abs() < 0.01, "{wrapper:?} / {text:?}");
        assert!(
            (wrapper.y + wrapper.h / 2.0 - icon.y - icon.h / 2.0).abs() < 0.01,
            "{wrapper:?} / {icon:?}"
        );
    }
    message.set("短标题".to_owned());
    app.settle().unwrap();
    assert_eq!(app.snapshot().find("wrapper").unwrap().frame.h, 21.0);
}

#[test]
fn growing_scroll_viewport_keeps_zero_main_basis_after_window_resize() {
    let mut app = TestApp::new((300.0, 500.0), || {
        column((
            Container::new().h(40.0).build(),
            column((scroll(column_fit((Container::new().h(900.0).build(),))),))
                .flex_grow(1.0)
                .automation_id("viewport-wrapper"),
        ))
    });
    for height in [500.0, 240.0, 700.0, 240.0] {
        app.resize(300.0, height).unwrap();
        app.settle().unwrap();
        let frame = app.snapshot().find("viewport-wrapper").unwrap().frame;
        assert!(
            (frame.h - (height - 40.0)).abs() < 0.01,
            "cached content must not become a main-axis basis: {frame:?}"
        );
    }
}

#[test]
fn space_and_layout_use_the_parent_direction_for_growing_cross_sizes() {
    for space in [true, false] {
        for direction in [FlexDirection::Row, FlexDirection::Column] {
            let app = TestApp::new((400.0, 160.0), move || {
                let children = vec![
                    ViewNode::new(
                        Container::new().flex_grow(1.0),
                        vec![Container::new().size(80.0, 20.0).build()],
                    )
                    .align_self(AlignItems::Center)
                    .automation_id("growing"),
                ];
                if space {
                    ViewNode::new(
                        Space::new().direction(direction).width(400.0).height(160.0),
                        children,
                    )
                } else {
                    ViewNode::new(Layout::new().direction(direction), children)
                }
            });
            let frame = app.snapshot().find("growing").unwrap().frame;
            let (cross, expected) = if matches!(direction, FlexDirection::Row) {
                (frame.h, 20.0)
            } else {
                (frame.w, 80.0)
            };
            assert!(
                (cross - expected).abs() < 0.01,
                "space={space} {direction:?}: {frame:?}"
            );
        }
    }
}

#[test]
fn growing_cross_axis_respects_padding_border_and_explicit_size() {
    for explicit in [0.0_f32, 60.0] {
        let app = TestApp::new((400.0, 160.0), move || {
            row((ViewNode::new(
                Container::new()
                    .flex_grow(1.0)
                    .h(explicit)
                    .padding(EdgeInsets::uniform(5.0))
                    .border(Color::BLACK, 2.0),
                vec![Container::new().size(80.0, 20.0).build()],
            )
            .automation_id("growing"),))
            .align(AlignItems::Center)
        });
        let frame = app.snapshot().find("growing").unwrap().frame;
        let expected = if explicit > 0.0 { 60.0 } else { 34.0 };
        assert!(
            (frame.h - expected).abs() < 0.01,
            "explicit={explicit} {frame:?}"
        );
        assert!((frame.y + frame.h / 2.0 - 80.0).abs() < 0.01, "{frame:?}");
    }
}
