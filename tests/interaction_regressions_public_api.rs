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

#[test]
fn modal_enter_scales_nested_visual_bounds_once_and_keeps_hit_testing() {
    fn scene(animated: bool, calls: State<u32>) -> TestApp {
        TestApp::new((640.0, 480.0), move || {
            let calls = calls.clone();
            Modal::builder()
                .open(&State::new(true))
                .size(480.0, 320.0)
                .enter_animation(AnimationConfig::zoom_in(if animated { 3.0 } else { 0.0 }))
                .content(move || {
                    column_fit((button("Nested action")
                        .on_click_fn(move || calls.update(|n| *n += 1))
                        .automation_id("nested-action"),))
                    .scale(0.75)
                    .automation_id("content-wrapper")
                })
                .build()
        })
    }
    let calls = State::new(0);
    let mut animated = scene(true, calls.clone());
    let stable = scene(false, State::new(0));
    // TestApp 不推进时钟；明确检查公开 zoom_in 的首个 0.8 缩放采样。
    let initial = animated.snapshot();
    let settled = stable.snapshot();
    for id in ["content-wrapper", "nested-action"] {
        let first = initial.find(id).unwrap();
        let last = settled.find(id).unwrap();
        // AutomationSnapshot.frame 已是视觉坐标，不冒充组件的原始布局 frame。
        assert!((first.frame.w / last.frame.w - 0.8).abs() < 0.001);
        assert!((first.frame.h / last.frame.h - 0.8).abs() < 0.001);
        let first = first.visible_bounds.unwrap();
        let last = last.visible_bounds.unwrap();
        assert!(
            (first.w / last.w - 0.8).abs() < 0.001,
            "只在父子边界缩放一次: {id}"
        );
        assert!((first.h / last.h - 0.8).abs() < 0.001);
    }
    animated.click("nested-action").unwrap();
    assert_eq!(calls.get(), 1, "真实坐标命中必须跟随组合后的缩放");
}

#[test]
fn modal_enter_close_slot_uses_the_same_scaled_geometry_as_paint() {
    let open = State::new(true);
    let root_open = open.clone();
    let mut app = TestApp::new((640.0, 480.0), move || {
        Modal::builder()
            .open(&root_open)
            .size(480.0, 320.0)
            .mask_closable(false)
            .enter_animation(AnimationConfig::zoom_in(3.0))
            .content(|| label("Body"))
            .build()
    });
    // 480x320 panel 居中；关闭槽中心 (536,108) 围绕 (320,240) 缩放 0.8。
    let pos = Point::new(492.8, 134.4);
    for event in [
        SystemEvent::PointerDown {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        },
        SystemEvent::PointerUp {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        },
    ] {
        app.dispatch_system_event(&event).unwrap();
    }
    assert!(
        !open.get(),
        "缩放后的关闭图标应命中，且不能由 mask_closable 冒充成功"
    );
}

#[test]
fn modal_openers_key_pairs_move_focus_into_content() {
    for opener in ["external-opener", "builtin-opener"] {
        for key in [KeyCode::Space, KeyCode::Enter] {
            let open = State::new(false);
            let changes = State::new(0_u32);
            let root_open = open.clone();
            let root_changes = changes.clone();
            let mut app = TestApp::new((640.0, 480.0), move || {
                let changes = root_changes.clone();
                let external_changes = root_changes.clone();
                let external_open = root_open.clone();
                column_fit((
                    input()
                        .placeholder("Previous focus")
                        .build()
                        .automation_id("previous"),
                    button("External opener")
                        .on_click_fn(move || {
                            external_changes.update(|n| *n += 1);
                            external_open.set(true);
                        })
                        .automation_id("external-opener"),
                    Modal::builder()
                        .open(&root_open)
                        .content(|| {
                            input()
                                .placeholder("Dialog input")
                                .build()
                                .automation_id("body-input")
                        })
                        .on_open_change(move |_| changes.update(|n| *n += 1))
                        .build()
                        .automation_id("builtin-opener"),
                ))
            });
            app.focus("previous").unwrap();
            // The native probe separately establishes this restored closed-state target.
            // Here test its public paired keys, without claiming TestApp advanced an exit.
            app.focus(opener).unwrap();
            app.press_key(key, KeyMod::NONE).unwrap();
            assert!(
                open.get(),
                "{opener} must accept its advertised keyboard activation"
            );
            assert_eq!(changes.get(), 1, "a paired key opens exactly once");
            assert!(
                app.snapshot().find("body-input").unwrap().focused,
                "the closed-state owner is an opener, not already-focused dialog content"
            );
            app.press_key(KeyCode::Escape, KeyMod::NONE).unwrap();
            assert!(!open.get());
        }
    }
}
