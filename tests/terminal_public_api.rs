#![cfg(all(feature = "test-harness", feature = "terminal"))]

use std::cell::RefCell;
use std::rc::Rc;
use uix::prelude::*;
use uix::ui::test_harness::TestApp;

#[test]
fn terminal_declared_size_updates_without_losing_the_command_draft() {
    let size = State::new((240.0_f32, 120.0_f32));
    let root_size = size.clone();
    let submitted = Rc::new(RefCell::new(String::new()));
    let observed = submitted.clone();
    let mut app = TestApp::new((640.0, 480.0), move || {
        let (width, height) = root_size.get();
        let observed = observed.clone();
        column((embed(Terminal::new().on_command(move |text| {
            *observed.borrow_mut() = text.to_owned();
        }))
        .width(width)
        .height(height)
        .flex_shrink(0.0)
        .automation_id("console"),))
        .width(600.0)
        .align(AlignItems::Start)
    });
    let frame = app.snapshot().find("console").unwrap().frame;
    assert_eq!((frame.w, frame.h), (240.0, 120.0));
    app.focus("console").unwrap();
    app.dispatch_system_event(&SystemEvent::TextInput {
        text: "草稿".to_owned(),
    })
    .unwrap();
    size.set((300.0, 160.0));
    app.settle().unwrap();
    let frame = app.snapshot().find("console").unwrap().frame;
    assert_eq!((frame.w, frame.h), (300.0, 160.0));
    app.press_key(KeyCode::Enter, KeyMod::NONE).unwrap();
    assert_eq!(&*submitted.borrow(), "草稿");
}

#[test]
fn terminal_participates_in_flex_layout() {
    let app = TestApp::new((600.0, 300.0), || {
        row((
            embed(Terminal::new())
                .width(100.0)
                .height(100.0)
                .flex_grow(1.0)
                .automation_id("grow"),
            embed(Terminal::new())
                .width(100.0)
                .height(100.0)
                .flex_grow(0.0)
                .flex_shrink(0.0)
                .automation_id("fixed"),
        ))
        .width(600.0)
    });
    assert_eq!(app.snapshot().find("grow").unwrap().frame.w, 500.0);
    assert_eq!(app.snapshot().find("fixed").unwrap().frame.w, 100.0);
}
