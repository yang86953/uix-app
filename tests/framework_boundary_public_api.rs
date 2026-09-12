//! A design-system author can use these contracts with uix-app alone.
#![cfg(all(feature = "ui", feature = "test-harness"))]
use uix_app::prelude::*;
use uix_app::ui::test_harness::TestApp;
use uix_app::ui::{
    ComponentContext, DynamicChildrenCoordinator, DynamicRefresh, ThemeTokens, TokenPatch,
};

#[derive(Clone, Default, PartialEq)]
struct Language(&'static str);
#[derive(Clone, Default, PartialEq)]
struct Density(u8);

#[test]
fn typed_contexts_inherit_independently_and_restore_after_unwind() {
    with_context(&Language("outer"), || {
        with_context(&Density(2), || {
            assert_eq!(use_context::<Language>().0, "outer");
            let before = use_context::<Language>();
            assert!(
                std::panic::catch_unwind(|| with_context(&Language("inner"), || {
                    assert_eq!(use_context::<Language>().0, "inner");
                    assert_eq!(use_context::<Density>().0, 2);
                    panic!("restore context");
                }))
                .is_err()
            );
            assert!(before == use_context::<Language>());
        });
        assert_eq!(use_context::<Density>().0, 0);
    });
    assert_eq!(use_context::<Language>().0, "");
}

struct IndependentTokens;
impl ThemeTokens for IndependentTokens {
    fn value(&self, key: &str) -> Option<TokenValue> {
        match key {
            "sample.accent" => Some(TokenValue::Color(Color::from_rgb(23, 45, 67))),
            "sample.unit" => Some(TokenValue::Number(9.0)),
            _ => None,
        }
    }
}
#[test]
fn open_tokens_and_overlays_keep_unmentioned_library_values() {
    let theme = Theme::new(IndependentTokens)
        .patched(TokenPatch::default().with("sample.unit", TokenValue::Number(13.0)));
    assert_eq!(theme.tokens().number("sample.unit", 0.0), 13.0);
    assert_eq!(
        ColorValue::token("sample.accent", Color::white()).resolve(theme.tokens()),
        Color::from_rgb(23, 45, 67)
    );
    assert_eq!(theme.tokens().number("other.library.unit", 17.0), 17.0);
}

#[test]
fn unicode_editing_preserves_graphemes_selection_and_length_limits() {
    let mut state = TextEditState::new("a👩‍💻e\u{301}中");
    state.edit(false, None).set_cursor(4);
    assert!(state.edit(false, None).delete_previous_grapheme());
    assert_eq!(state.value(), "ae\u{301}中");
    assert_eq!(state.cursor(), 1);
    state.edit(false, None).move_cursor_right(false, true);
    assert_eq!(state.selection(), Some((1, 3)));
    assert!(state.edit(false, None).insert_text_at_cursor("文"));
    assert_eq!(state.value(), "a文中");
    state.edit(false, None).set_cursor(3);
    assert!(!state.edit(false, Some(3)).insert_text_at_cursor("\nX"));
    assert_eq!(state.value(), "a文中");
    assert_eq!(TextEditState::slice("a👩‍💻z", 2, 3), "👩‍💻");
}

#[test]
fn neutral_primitives_measure_grid_tracks_and_scroll_without_components() {
    let app = TestApp::new((240.0, 160.0), || {
        let mut style = Style::default()
            .with_display(DisplayMode::Grid)
            .with_grid_columns(vec![GridTrack::Px(60.0), GridTrack::Px(90.0)])
            .with_grid_rows(vec![GridTrack::Px(30.0)]);
        style.width = Some(160.0);
        style.height = Some(40.0);
        style.grid_column_gap = 10.0;
        ViewNode::new(
            Container::new().style(style),
            vec![
                label("one").automation_id("one"),
                label("two").automation_id("two"),
            ],
        )
    });
    let snapshot = app.snapshot();
    assert!((snapshot.find("one").unwrap().frame.x - 0.0).abs() < 0.01);
    assert!((snapshot.find("two").unwrap().frame.x - 70.0).abs() < 0.01);
    let scroll = TestApp::new((120.0, 60.0), || {
        uix_app::ui::scroll((column_fit((label("first"), label("second"))),))
    });
    assert!(
        scroll
            .snapshot()
            .nodes
            .iter()
            .any(|node| node.accessibility.name.as_deref() == Some("second"))
    );
}

widget! {
    struct Deferred {}
    @new -> Self { Self {} }
    dynamic_children => (&COORDINATOR) {}
    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &uix_app::ui::__private::WidgetTree) {}
}
struct Coordinator;
static COORDINATOR: Coordinator = Coordinator;
impl DynamicChildrenCoordinator for Coordinator {
    fn refresh(
        &self,
        context: &mut ComponentContext<'_>,
        phase: DynamicRefresh,
        _: Option<f32>,
    ) -> bool {
        if phase != DynamicRefresh::Mount {
            return false;
        }
        if let Some(parent) = context.get(context.owner()).unwrap().parent() {
            assert!(
                context.get(parent).is_none(),
                "the parent is outside the component scope"
            );
        }
        let captured = context.capture_context().capture("content", "stable", || {
            label(use_context::<Language>().0).automation_id("captured")
        });
        context.reconcile_children(vec![captured])
    }
}
#[test]
fn delayed_children_capture_the_owners_typed_context() {
    let app = TestApp::new((100.0, 50.0), || {
        column((ContextProvider::new(Language("local"))
            .child(|| ViewNode::leaf(Deferred::new()))
            .build(),))
    });
    assert_eq!(
        app.snapshot()
            .find("captured")
            .unwrap()
            .accessibility
            .name
            .as_deref(),
        Some("local")
    );
    assert_eq!(use_context::<Language>().0, "");
}
