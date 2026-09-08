//! Neutral public-API reproductions of consumer scrolling, Tabs and Modal reports.
//! TestApp exercises semantics and logical geometry, not GPU/window presentation.

#![cfg(all(
    feature = "navigation",
    feature = "feedback",
    feature = "agent-control",
    feature = "test-harness"
))]

use uix::core::Point;
use uix::prelude::*;
use uix::ui::test_harness::TestApp;

#[test]
fn keyed_virtual_scroll_last_row_is_visible_before_invocation() {
    let mut app = TestApp::new((400.0, 600.0), || {
        VirtualScroll::new()
            .item_count(1000)
            .item_height(48.0)
            .size(400.0, 600.0)
            .render_keyed(
                |index| format!("item-{index}"),
                |index| {
                    button(format!("Item {index}"))
                        .height(48.0)
                        .automation_id(format!("row-{index}"))
                },
            )
            .build()
            .automation_id("list")
    });
    for delta in [-100_000.0, 480.0, -100_000.0] {
        app.scroll("list", Point::new(0.0, delta)).unwrap();
        if delta < 0.0 {
            let snapshot = app.snapshot();
            let last = snapshot.find("row-999").expect("last row materializes");
            let visible = last.visible_bounds.expect("last row visible before invoke");
            assert!(visible.h > 0.0 && visible.w > 0.0, "{last:?}");
            assert!(last.frame.y + last.frame.h <= 600.1, "{last:?}");
        }
    }
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
fn tabs_public_select_updates_controlled_state() {
    let active = State::new("one".to_owned());
    let bound = active.clone();
    let mut app = TestApp::new((300.0, 240.0), move || {
        embed(
            Tabs::new()
                .tabs(vec![Tab::new("One").key("one"), Tab::new("Two").key("two")])
                .active_key(&bound),
        )
        .automation_id("tabs")
    });
    app.select("tabs", 1)
        .expect("public semantic selection supported");
    assert_eq!(active.get(), "two");
}

#[test]
fn modal_enter_confirms_and_escape_cancels_controlled_open() {
    for key in [KeyCode::Enter, KeyCode::Escape] {
        let open = State::new(true);
        let ok_count = State::new(0_u32);
        let cancel_count = State::new(0_u32);
        let root_open = open.clone();
        let root_ok = ok_count.clone();
        let root_cancel = cancel_count.clone();
        let mut app = TestApp::new((640.0, 480.0), move || {
            let ok = root_ok.clone();
            let cancel = root_cancel.clone();
            Modal::builder()
                .open(&root_open)
                .title("Neutral dialog")
                .content(|| label("Body").automation_id("dialog-body"))
                .footer_visible(true)
                .on_ok(move || ok.set(ok.get() + 1))
                .on_cancel(move || cancel.set(cancel.get() + 1))
                .build()
                .automation_id("dialog")
        });
        assert!(
            app.snapshot()
                .find("dialog-body")
                .unwrap()
                .visible_bounds
                .is_some()
        );
        // The overlay establishes keyboard focus; do not request focus on a zero-size root.
        app.press_key(key, KeyMod::NONE).unwrap();
        assert!(!open.get(), "dialog close writes controlled state");
        assert_eq!(ok_count.get(), u32::from(key == KeyCode::Enter));
        assert_eq!(cancel_count.get(), u32::from(key == KeyCode::Escape));
    }
}

#[test]
fn tabs_selection_tracks_panels_and_rejects_invalid_indices() {
    let active = State::new("one".to_owned());
    let bound = active.clone();
    let mut app = TestApp::new((480.0, 300.0), move || {
        ViewNode::new(
            Tabs::new()
                .tab("One", "one")
                .tab("Two", "two")
                .tab("Three", "three")
                .active_key(&bound),
            vec![
                button("First").automation_id("panel-one"),
                button("Second").automation_id("panel-two"),
                button("Third").automation_id("panel-three"),
            ],
        )
        .automation_id("tabs")
    });
    for (index, key, panel) in [
        (2, "three", "panel-three"),
        (0, "one", "panel-one"),
        (1, "two", "panel-two"),
        (1, "two", "panel-two"),
    ] {
        app.select("tabs", index).unwrap();
        assert_eq!(active.get(), key);
        let snapshot = app.snapshot();
        let tabs = snapshot.find("tabs").unwrap();
        assert_eq!(
            tabs.selection.as_ref().unwrap().selected_indices,
            vec![index]
        );
        assert!(snapshot.find(panel).unwrap().visible_bounds.is_some());
    }
    assert!(app.select("tabs", 3).is_err());
    assert_eq!(active.get(), "two");
}

#[test]
fn empty_tabs_does_not_advertise_select() {
    let mut app = TestApp::new((400.0, 240.0), || embed(Tabs::new()).automation_id("tabs"));
    assert!(
        !app.snapshot()
            .find("tabs")
            .unwrap()
            .actions
            .contains(&uix::ui::SemanticActionKind::Select)
    );
    assert!(
        app.snapshot()
            .find("tabs")
            .unwrap()
            .selection
            .as_ref()
            .unwrap()
            .options
            .is_empty()
    );
    assert!(app.select("tabs", 0).is_err());
}

#[test]
fn opening_modal_moves_keyboard_away_from_background() {
    let open = State::new(false);
    let background = State::new(0);
    let confirmed = State::new(0);
    let root_open = open.clone();
    let root_background = background.clone();
    let root_confirmed = confirmed.clone();
    let mut app = TestApp::new((640.0, 480.0), move || {
        let background = root_background.clone();
        let confirmed = root_confirmed.clone();
        column((
            button("Background")
                .on_click_fn(move || background.set(background.get() + 1))
                .automation_id("background"),
            Modal::builder()
                .open(&root_open)
                .content(|| label("Body"))
                .footer_visible(true)
                .on_ok(move || confirmed.set(confirmed.get() + 1))
                .build()
                .automation_id("dialog"),
        ))
    });
    app.focus("background").unwrap();
    open.set(true);
    app.settle().unwrap();
    assert!(app.snapshot().find("dialog").unwrap().focused);
    app.press_key(KeyCode::Enter, KeyMod::NONE).unwrap();
    assert_eq!(background.get(), 0);
    assert_eq!(confirmed.get(), 1);
    assert!(!open.get());
}

#[test]
fn modal_prefers_content_focus_without_stealing_its_activation() {
    let open = State::new(true);
    let activated = State::new(0);
    let confirmed = State::new(0);
    let root_open = open.clone();
    let root_activated = activated.clone();
    let root_confirmed = confirmed.clone();
    let mut app = TestApp::new((640.0, 480.0), move || {
        let activated = root_activated.clone();
        let confirmed = root_confirmed.clone();
        Modal::builder()
            .open(&root_open)
            .content(move || {
                button("Content action")
                    .on_click_fn(move || activated.set(activated.get() + 1))
                    .automation_id("content")
            })
            .footer_visible(true)
            .on_ok(move || confirmed.set(confirmed.get() + 1))
            .build()
            .automation_id("dialog")
    });
    assert!(app.snapshot().find("content").unwrap().focused);
    app.press_key(KeyCode::Enter, KeyMod::NONE).unwrap();
    assert_eq!(activated.get(), 1);
    assert_eq!(confirmed.get(), 0);
    assert!(open.get());
    app.press_key(KeyCode::Escape, KeyMod::NONE).unwrap();
    assert!(!open.get());
}

#[test]
fn modal_hidden_footer_does_not_confirm_on_enter() {
    let open = State::new(true);
    let root_open = open.clone();
    let mut app = TestApp::new((640.0, 480.0), move || {
        Modal::builder()
            .open(&root_open)
            .content(|| label("Body"))
            .footer_visible(false)
            .build()
    });
    app.press_key(KeyCode::Enter, KeyMod::NONE).unwrap();
    assert!(open.get());
    app.press_key(KeyCode::Escape, KeyMod::NONE).unwrap();
    assert!(!open.get());
}
