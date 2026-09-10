//! Public Tabs layout contract, observed through logical geometry (not GPU pixels).
#![cfg(all(
    feature = "navigation",
    feature = "agent-control",
    feature = "test-harness"
))]
use uix_app::prelude::*;
use uix_app::ui::test_harness::TestApp;

fn tabs() -> ViewNode {
    embed(Tabs::new().tab("One", "one").tab("Two", "two")).automation_id("tabs")
}

fn close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.1, "{actual} != {expected}");
}

#[test]
fn tabs_explicit_flex_grow_fills_remaining_column() {
    let app = TestApp::new((300.0, 240.0), || {
        column_fit((
            label("Header").automation_id("header"),
            embed(Tabs::new().tabs(vec![Tab::new("One").key("one"), Tab::new("Two").key("two")]))
                .flex_grow(1.0)
                .automation_id("tabs"),
        ))
        .height(240.0)
    });
    let snapshot = app.snapshot();
    let header = snapshot.find("header").unwrap();
    let tabs = snapshot.find("tabs").unwrap();
    assert!(
        (tabs.frame.y + tabs.frame.h - 240.0).abs() < 0.1,
        "{tabs:?}"
    );
    assert!((tabs.frame.y - header.frame.y - header.frame.h).abs() < 0.1);
}

#[test]
fn tabs_default_and_explicit_zero_grow_keep_natural_height() {
    for zero in [false, true] {
        let app = TestApp::new((300.0, 400.0), move || {
            column((
                label("Header"),
                if zero { tabs().flex_grow(0.0) } else { tabs() },
            ))
        });
        close(app.snapshot().find("tabs").unwrap().frame.h, 200.0);
    }
}

#[test]
fn tabs_view_dimensions_override_builder_dimensions() {
    let app = TestApp::new((500.0, 400.0), || {
        row((embed(Tabs::new().tab("One", "one").size(120.0, 100.0))
            .width(180.0)
            .height(150.0)
            .automation_id("tabs"),))
        .align(AlignItems::Start)
    });
    let frame = app.snapshot().find("tabs").unwrap().frame;
    close(frame.w, 180.0);
    close(frame.h, 150.0);
}

#[test]
fn tabs_explicit_shrink_controls_overflow() {
    for shrink in [0.0, 1.0] {
        let app = TestApp::new((300.0, 100.0), move || {
            column((tabs().flex_shrink(shrink),)).height(100.0)
        });
        close(
            app.snapshot().find("tabs").unwrap().frame.h,
            if shrink == 0.0 { 200.0 } else { 100.0 },
        );
    }
}

#[test]
fn tabs_grow_tracks_viewport_resize_in_both_axes() {
    let mut app = TestApp::new((400.0, 300.0), || column((tabs().flex_grow(1.0),)));
    for (w, h) in [(500.0, 450.0), (320.0, 240.0)] {
        app.resize(w, h).unwrap();
        let frame = app.snapshot().find("tabs").unwrap().frame;
        close(frame.w, w);
        close(frame.h, h);
    }
    let app = TestApp::new((500.0, 240.0), || {
        row((label("Sidebar").width(100.0), tabs().flex_grow(1.0))).width(500.0)
    });
    let frame = app.snapshot().find("tabs").unwrap().frame;
    close(frame.x, 100.0);
    close(frame.w, 400.0);
}

#[test]
fn tabs_rebuild_updates_and_removes_flex_without_losing_selection() {
    let phase = State::new(0);
    let bound = phase.clone();
    let mut app = TestApp::new((300.0, 400.0), move || {
        let node = match bound.get() {
            0 | 2 => tabs().flex_grow(1.0),
            1 => tabs().flex_grow(0.0),
            _ => tabs(),
        };
        column((node,))
    });
    app.focus("tabs").unwrap();
    app.press_key(KeyCode::Right, KeyMod::NONE).unwrap();
    let id = app.snapshot().find("tabs").unwrap().id;
    assert_eq!(app.text("tabs").unwrap(), "Two");
    for (next, height) in [(1, 200.0), (2, 400.0), (3, 200.0)] {
        phase.set(next);
        app.settle().unwrap();
        let snap = app.snapshot();
        let node = snap.find("tabs").unwrap();
        assert_eq!(node.id, id);
        close(node.frame.h, height);
        assert_eq!(app.text("tabs").unwrap(), "Two");
    }
}

#[test]
fn tabs_rebuild_updates_and_removes_explicit_dimensions() {
    let sized = State::new(true);
    let bound = sized.clone();
    let mut app = TestApp::new((500.0, 400.0), move || {
        row((if bound.get() {
            tabs().width(180.0).height(150.0)
        } else {
            tabs()
        },))
        .align(AlignItems::Start)
    });
    close(app.snapshot().find("tabs").unwrap().frame.h, 150.0);
    sized.set(false);
    app.settle().unwrap();
    let frame = app.snapshot().find("tabs").unwrap().frame;
    close(frame.w, 400.0);
    close(frame.h, 200.0);
}

#[test]
fn tabs_rebuild_updates_and_removes_shrink_override() {
    let phase = State::new(0);
    let bound = phase.clone();
    let mut app = TestApp::new((300.0, 100.0), move || {
        column((match bound.get() {
            0 | 2 => tabs().flex_shrink(0.0),
            1 => tabs().flex_shrink(1.0),
            _ => tabs(),
        },))
    });
    close(app.snapshot().find("tabs").unwrap().frame.h, 200.0);
    for (next, height) in [(1, 100.0), (2, 200.0), (3, 100.0)] {
        phase.set(next);
        app.settle().unwrap();
        close(app.snapshot().find("tabs").unwrap().frame.h, height);
    }
}

#[test]
fn tabs_builder_flex_survives_unrelated_style_and_accepts_explicit_zero() {
    for zero in [false, true] {
        let app = TestApp::new((300.0, 400.0), move || {
            let node = embed(Tabs::new().tab("One", "one").flex_grow(1.0))
                .width(280.0)
                .automation_id("builder.tabs");
            column((if zero { node.flex_grow(0.0) } else { node },))
        });
        close(app.snapshot().find("builder.tabs").unwrap().frame.h, if zero { 200.0 } else { 400.0 });
    }
}

#[test]
fn tabs_builder_shrink_survives_unrelated_style_and_accepts_override() {
    for shrink in [None, Some(1.0)] {
        let app = TestApp::new((300.0, 100.0), move || {
            let node = embed(Tabs::new().tab("One", "one").flex_shrink(0.0))
                .width(280.0)
                .automation_id("builder.tabs");
            column((if let Some(value) = shrink { node.flex_shrink(value) } else { node },))
        });
        close(app.snapshot().find("builder.tabs").unwrap().frame.h, if shrink.is_none() { 200.0 } else { 100.0 });
    }
}
