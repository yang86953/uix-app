//! External UIX Lang consumer regression; logical semantics only, no GPU/window checks.
#![cfg(feature = "test-harness")]
#![allow(non_snake_case, unused_braces)]

use uix::prelude::*;
use uix::ui::test_harness::{AutomationSnapshot, TestApp};

uix_items!("examples/fixtures/vscroll_report.uix");

fn rows(revision: u32, count: usize) -> Vec<ProbeRow> {
    (0..count)
        .map(|i| ProbeRow {
            id: format!("TASK-{i:04}"),
            title: format!("Task {i:04} / revision {revision}"),
            statusLabel: "Todo".to_owned(),
            priority: "High".to_owned(),
            due: "2026-09-08".to_owned(),
            automationId: format!("task-TASK-{i:04}"),
        })
        .collect()
}

fn fixture() -> (TestApp, State<Vec<ProbeRow>>, State<String>) {
    let probeRows = State::new(rows(0, 1000));
    let selectedTaskId = State::new(String::new());
    let data = probeRows.clone();
    let selected = selectedTaskId.clone();
    let probeStatus = State::new("Synthetic consumer".to_owned());
    let probeShrinkTrigger = State::new(false);
    let probeRestoreTrigger = State::new(false);
    let app = TestApp::new((1440.0, 900.0), move || {
        uix!("examples/fixtures/vscroll_report.uix")
    });
    (app, data, selected)
}

fn visible_row_and_title(snapshot: &AutomationSnapshot, index: usize, revision: u32) {
    let row = snapshot.find(&format!("task-TASK-{index:04}")).unwrap();
    let bounds = row
        .visible_bounds
        .expect("row must be visible before invoke");
    let viewport = snapshot.find("probe-list").unwrap().visible_bounds.unwrap();
    assert!((row.frame.h - 48.0).abs() < 0.1, "{row:?}");
    assert!(bounds.w > 0.0 && bounds.h > 0.0, "{row:?}");
    assert!(bounds.y + bounds.h <= viewport.y + viewport.h + 0.1);
    let expected = format!("Task {index:04} / revision {revision}");
    let title = snapshot
        .nodes
        .iter()
        .find(|node| node.accessibility.name.as_deref() == Some(expected.as_str()))
        .unwrap();
    let text_bounds = title
        .visible_bounds
        .expect("updated title must remain visible");
    assert!(text_bounds.w > 0.0 && text_bounds.h > 0.0, "{title:?}");
}

#[test]
fn conditional_row_template_reaches_tail_before_selection_and_after_roundtrip() {
    let (mut app, _, selected) = fixture();
    app.perform("task-TASK-0000", SemanticAction::Invoke)
        .unwrap();
    assert_eq!(selected.get(), "TASK-0000");
    // TestApp accepts native wheel sign; the direct semantic action uses positive-down.
    app.scroll("probe-list", Point::new(0.0, -576.0)).unwrap();
    app.perform("task-TASK-0480", SemanticAction::Invoke)
        .unwrap();
    assert_eq!(selected.get(), "TASK-0480");
    for delta in [-100000.0, 12.0, -100000.0] {
        app.scroll("probe-list", Point::new(0.0, delta)).unwrap();
        if delta < 0.0 {
            visible_row_and_title(&app.snapshot(), 999, 0);
            assert_eq!(
                selected.get(),
                "TASK-0480",
                "scroll must not select the tail to heal it"
            );
        }
    }
    app.perform("task-TASK-0999", SemanticAction::Invoke)
        .unwrap();
    assert_eq!(selected.get(), "TASK-0999");
    visible_row_and_title(&app.snapshot(), 999, 0);
}

#[test]
fn refreshed_titles_and_data_clamping_remain_visible_with_stable_keys() {
    let (mut app, data, selected) = fixture();
    app.scroll("probe-list", Point::new(0.0, -100000.0))
        .unwrap();
    visible_row_and_title(&app.snapshot(), 999, 0);
    data.set(rows(1, 1000));
    app.settle().unwrap();
    visible_row_and_title(&app.snapshot(), 999, 1);
    app.scroll("probe-list", Point::new(0.0, 600.0)).unwrap();
    data.set(rows(2, 1000));
    app.settle().unwrap();
    visible_row_and_title(&app.snapshot(), 499, 2);
    app.scroll("probe-list", Point::new(0.0, -100000.0))
        .unwrap();
    app.perform("task-TASK-0999", SemanticAction::Invoke)
        .unwrap();
    data.set(rows(2, 600));
    app.settle().unwrap();
    visible_row_and_title(&app.snapshot(), 599, 2);
    assert_eq!(
        selected.get(),
        "TASK-0999",
        "data replacement must not take over controlled selection"
    );
    assert!(app.snapshot().find("task-TASK-0999").is_err());
}
