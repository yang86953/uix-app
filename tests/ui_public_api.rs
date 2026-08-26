use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use uix::ui::{
    DesignTokens, IBoxShadowTokens, IColorTokens, ISpacingTokens, ITypographyTokens, State,
    ThemeTokens, TokenPatch, TokenProvider,
};
use uix::ui::{StyleExt, button, column, label};

#[test]
fn theme_and_token_contracts_are_owned_by_ui() {
    fn assert_theme_provider<T>()
    where
        T: IColorTokens
            + ITypographyTokens
            + ISpacingTokens
            + IBoxShadowTokens
            + ThemeTokens
            + TokenProvider,
    {
    }

    assert_theme_provider::<DesignTokens>();
    let _patch = TokenPatch::default();
}

#[test]
fn ui_state_and_view_builders_work_from_public_exports() {
    let count = State::new(0_u32);
    let notifications = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&notifications);
    count.watch(move |_| {
        observed.fetch_add(1, Ordering::Relaxed);
    });
    count.update(|value| *value += 1);
    assert_eq!(count.get(), 1);
    assert_eq!(notifications.load(Ordering::Relaxed), 1);

    let event_count = count.clone();
    let _root = column((
        label("Count").automation_id("counter.label"),
        button("+1")
            .on_click(&event_count, |state| state.update(|value| *value += 1))
            .automation_id("counter.increment"),
    ));
}

#[cfg(feature = "test-harness")]
#[test]
fn ui_test_harness_drives_the_same_public_semantic_path() {
    use uix::ui::test_harness::TestApp;

    let count = State::new(0_u32);
    let root_count = count.clone();
    let mut app = TestApp::new((320.0, 200.0), move || {
        let click_count = root_count.clone();
        column((
            root_count
                .map_text(|value| value.to_string())
                .automation_id("counter.label"),
            button("+1")
                .on_click(&click_count, |state| state.update(|value| *value += 1))
                .automation_id("counter.increment"),
        ))
    });

    app.click("counter.increment")
        .expect("public semantic click should settle");
    assert_eq!(count.get(), 1);
    assert_eq!(app.text("counter.label").as_deref(), Ok("1"));
}

#[cfg(feature = "test-harness")]
#[test]
fn button_interaction_gate_preserves_accessibility_override_precedence() {
    use uix::ui::test_harness::{AutomationError, TestApp};
    use uix::ui::{AccessibilityExt, AccessibilityRole, AccessibilitySnapshot, AccessibilityState};

    let count = State::new(0_u32);
    let root_count = count.clone();
    let mut app = TestApp::new((320.0, 320.0), move || {
        let enabled_count = root_count.clone();
        let disabled_count = root_count.clone();
        let loading_count = root_count.clone();
        let state_count = root_count.clone();
        let replacement_count = root_count.clone();
        column((
            button("可用")
                .on_click(&enabled_count, |state| state.update(|value| *value += 1))
                .accessible_name("仍可用")
                .automation_id("button.enabled"),
            button("组件禁用")
                .disabled(true)
                .on_click(&disabled_count, |state| state.update(|value| *value += 10))
                .automation_id("button.disabled"),
            button("加载中")
                .loading(true)
                .on_click(&loading_count, |state| state.update(|value| *value += 100))
                .automation_id("button.loading"),
            button("状态禁用")
                .on_click(&state_count, |state| state.update(|value| *value += 1_000))
                .accessibility_state(AccessibilityState::disabled(true))
                .automation_id("button.state-disabled"),
            button("快照禁用")
                .on_click(&replacement_count, |state| {
                    state.update(|value| *value += 10_000)
                })
                .accessibility(
                    AccessibilitySnapshot::named(AccessibilityRole::Button, "快照禁用")
                        .with_state(AccessibilityState::disabled(true)),
                )
                .automation_id("button.snapshot-disabled"),
        ))
    });

    app.click("button.enabled").expect("仅覆盖名称不应禁用按钮");
    for automation_id in [
        "button.disabled",
        "button.loading",
        "button.state-disabled",
        "button.snapshot-disabled",
    ] {
        assert_eq!(
            app.click(automation_id),
            Err(AutomationError::Disabled(automation_id.to_owned()))
        );
    }
    assert_eq!(count.get(), 1);
}
