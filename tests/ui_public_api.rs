use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use uix::ui::view::{button, column, label, StyleExt};
use uix::ui::{
    DesignTokens, IBoxShadowTokens, IColorTokens, ISpacingTokens, ITypographyTokens, State,
    ThemeTokens, TokenPatch, TokenProvider,
};

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
