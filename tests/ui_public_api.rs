use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use uix::ui::{
    DesignTokens, IBoxShadowTokens, IColorTokens, ISpacingTokens, ITypographyTokens, State,
    ThemeTokens, TokenPatch, TokenProvider,
};
use uix::ui::{StyleExt, button, column, embed, label, row, scroll};

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

#[cfg(feature = "test-harness")]
#[test]
fn root_flex_grow_chain_fills_and_refollows_viewport() {
    use uix::ui::test_harness::TestApp;

    let mut app = TestApp::new((420.0, 280.0), || {
        column((column((label("内容"),))
            .flex_grow(1.0)
            .automation_id("layout.middle"),))
        .flex_grow(1.0)
        .automation_id("layout.root")
    });

    let initial = app.snapshot();
    let root = initial.find("layout.root").expect("根布局应存在");
    let middle = initial.find("layout.middle").expect("中间布局应存在");
    assert!((root.frame.h - 280.0).abs() < 0.01);
    assert!((middle.frame.h - 280.0).abs() < 0.01);

    app.resize(640.0, 360.0).expect("视口重排应完成");
    let resized = app.snapshot();
    let root = resized.find("layout.root").expect("根布局应仍存在");
    let middle = resized.find("layout.middle").expect("中间布局应仍存在");
    assert!((root.frame.w - 640.0).abs() < 0.01);
    assert!((root.frame.h - 360.0).abs() < 0.01);
    assert!((middle.frame.w - 640.0).abs() < 0.01);
    assert!((middle.frame.h - 360.0).abs() < 0.01);
}

#[cfg(feature = "test-harness")]
#[test]
fn flex_children_honor_their_public_shrink_factors() {
    use uix::prelude::Container;
    use uix::ui::test_harness::TestApp;

    let app = TestApp::new((100.0, 40.0), || {
        row((
            embed(Container::new().size(80.0, 40.0)).automation_id("layout.shrinkable"),
            embed(Container::new().size(80.0, 40.0).flex_shrink(0.0)).automation_id("layout.fixed"),
        ))
        .width(100.0)
        .height(40.0)
    });

    let snapshot = app.snapshot();
    let shrinkable = snapshot
        .find("layout.shrinkable")
        .expect("默认可收缩子项应存在");
    let fixed = snapshot
        .find("layout.fixed")
        .expect("显式禁止收缩子项应存在");
    assert!((shrinkable.frame.w - 20.0).abs() < 0.01);
    assert!((fixed.frame.w - 80.0).abs() < 0.01);
}

#[cfg(feature = "test-harness")]
#[test]
fn space_children_honor_their_public_shrink_factors() {
    use uix::prelude::{Container, Space};
    use uix::ui::test_harness::TestApp;

    let app = TestApp::new((100.0, 40.0), || {
        embed(
            Space::new()
                .width(100.0)
                .height(40.0)
                .child(Container::new().size(80.0, 40.0))
                .child(Container::new().size(80.0, 40.0).flex_shrink(0.0)),
        )
        .automation_id("layout.space")
    });

    let snapshot = app.snapshot();
    let space = snapshot.find("layout.space").expect("Space 根节点应存在");
    let children: Vec<_> = snapshot
        .nodes
        .iter()
        .filter(|node| node.parent == Some(space.id))
        .collect();
    assert_eq!(children.len(), 2);
    assert!(children[0].frame.w < 80.0);
    assert!((children[1].frame.w - 80.0).abs() < 0.01);
}

#[cfg(feature = "test-harness")]
#[test]
fn bidirectional_scroll_preserves_fixed_content_across_coupled_scrollbars() {
    use uix::prelude::Container;
    use uix::ui::test_harness::TestApp;

    let app = TestApp::new((100.0, 100.0), || {
        scroll(embed(Container::new().size(100.0, 120.0)).automation_id("layout.scroll-content"))
            .both()
            .size(100.0, 100.0)
            .flex_grow(0.0)
            .into()
    });

    let snapshot = app.snapshot();
    let content = snapshot
        .find("layout.scroll-content")
        .expect("双向滚动内容应存在");
    assert!((content.frame.w - 100.0).abs() < 0.01);
    assert!((content.frame.h - 120.0).abs() < 0.01);
}

#[cfg(feature = "test-harness")]
#[test]
fn variable_virtual_scroll_uses_materialized_row_measurements() {
    use uix::prelude::VirtualScroll;
    use uix::ui::test_harness::TestApp;

    let app = TestApp::new((100.0, 100.0), || {
        VirtualScroll::new()
            .item_count(2)
            .item_height(10.0)
            .variable_height()
            .size(100.0, 100.0)
            .render(|index| {
                label(format!("行 {index}"))
                    .height(if index == 0 { 30.0 } else { 40.0 })
                    .automation_id(format!("layout.virtual-row-{index}"))
            })
            .into()
    });

    let snapshot = app.snapshot();
    let first = snapshot
        .find("layout.virtual-row-0")
        .expect("第一条可变高虚拟行应存在");
    let second = snapshot
        .find("layout.virtual-row-1")
        .expect("第二条可变高虚拟行应存在");
    assert!((first.frame.h - 30.0).abs() < 0.01);
    assert!((second.frame.y - first.frame.y - 30.0).abs() < 0.01);
    assert!((second.frame.h - 40.0).abs() < 0.01);
}

#[cfg(feature = "test-harness")]
#[test]
fn tabs_flex_grow_fills_remaining_column_height() {
    use uix::prelude::{Tab, Tabs, column_fit, embed, label};
    use uix::ui::test_harness::TestApp;

    // 契约：column_fit 中声明 flexGrow=1 的 Tabs 底边必须到达 Column 底边，
    // 面板随 Tabs 拉伸；页头保持固有高度并位于 Tabs 之上。
    let app = TestApp::new((300.0, 200.0), move || {
        // 根 column_fit 显式定高：模拟真实窗口链路中被父级拉伸的页面 Column，
        // 只有此时 Flex 剩余空间才存在并按 flexGrow 分配给 Tabs。
        column_fit((
            label("页头").automation_id("tabs.header"),
            embed(
                Tabs::new()
                    .tabs(vec![Tab::new("一").key("a"), Tab::new("二").key("b")])
                    .active(0)
                    .flex_grow(1.0),
            )
            .automation_id("tabs.stretched"),
        ))
        .height(200.0)
    });
    let snapshot = app.snapshot();
    let header = snapshot.find("tabs.header").expect("页头应存在");
    let tabs = snapshot
        .find("tabs.stretched")
        .expect("声明扩张的 Tabs 应存在");
    assert!(
        (tabs.frame.y + tabs.frame.h - 200.0).abs() < 0.01,
        "声明扩张的 Tabs 底边应到达 Column 底边：{:?}",
        tabs.frame
    );
    assert!(
        (tabs.frame.y - (header.frame.y + header.frame.h)).abs() < 0.01,
        "Tabs 应紧贴页头之下开始：header={:?} tabs={:?}",
        header.frame,
        tabs.frame
    );
}
