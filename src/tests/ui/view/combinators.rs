use super::*;
use crate::core::EdgeInsets;
use crate::ui::view::View;
use crate::ui::widgets::{Grid, ScrollView};

#[test]
fn grid_combinator_builds_grid_node() {
    let node = grid([label("A"), label("B")])
        .columns(vec![GridTrack::Fr(2.0), GridTrack::Px(120.0)])
        .rows(vec![GridTrack::Auto])
        .gap(8.0)
        .build();

    assert_eq!(node.children.len(), 2);
    assert!(node.widget.as_any().downcast_ref::<Grid>().is_some());
    assert_eq!(node.style.display, DisplayMode::Grid);
    assert_eq!(
        node.style.grid_template_columns,
        vec![GridTrack::Fr(2.0), GridTrack::Px(120.0)]
    );
    assert_eq!(node.style.grid_template_rows, vec![GridTrack::Auto]);
    assert_eq!(node.style.grid_column_gap, 8.0);
    assert_eq!(node.style.grid_row_gap, 8.0);
}

#[test]
fn scroll_combinator_builds_scroll_view_node() {
    let node = scroll(column([label("A")])).horizontal().build();

    assert_eq!(node.children.len(), 1);
    assert!(node.widget.as_any().downcast_ref::<ScrollView>().is_some());
}

#[test]
fn view_node_direct_style_methods_cover_margin_and_opacity() {
    let node = label("styled")
        .margin(EdgeInsets::uniform(6.0))
        .opacity(0.5);

    assert_eq!(node.style.margin, EdgeInsets::uniform(6.0));
    assert_eq!(node.style.opacity, 0.5);
}

#[test]
fn view_node_overflow_content_preserves_natural_flow() {
    let node = column([label("content")]).overflow_content();

    assert!(node.style.overflow_content);
}

#[test]
fn dynamic_label_measure_clamps_current_text() {
    use crate::core::{Constraints, Size};
    use crate::ui::traits::WidgetLayout;

    let label = DynamicLabel::new(|| "abcdef".to_string());

    let measured = label.measure(Constraints::loose(Size::new(30.0, 12.0)));

    assert_eq!(measured, Size::new(30.0, 12.0));
}

#[test]
fn column_accepts_heterogeneous_tuple_children() {
    let node = column((
        label("static"),
        button("go").on_click_fn(|| {}),
    ));
    assert_eq!(node.children.len(), 2);
}

#[test]
fn label_accepts_static_and_dynamic_content() {
    let static_node = label("hello");
    assert!(static_node
        .widget
        .as_any()
        .downcast_ref::<crate::ui::widgets::Label>()
        .is_some());

    let dynamic_node = label(|| "world".to_string());
    assert!(dynamic_node
        .widget
        .as_any()
        .downcast_ref::<DynamicLabel>()
        .is_some());
}

#[test]
fn state_map_text_builds_dynamic_label() {
    let count = crate::ui::state::State::new(7);
    let node = count.map_text(|n| format!("n={n}"));
    assert!(node
        .widget
        .as_any()
        .downcast_ref::<DynamicLabel>()
        .is_some());
}

#[test]
fn button_on_click_attaches_state_capture_fingerprint() {
    let count = crate::ui::state::State::new(0);
    let node = View::build(button("+1").on_click(&count, |c| c.update(|v| *v += 1)));
    assert_eq!(node.handlers.len(), 1);
    assert!(node.handlers[0].signature().capture_fingerprint.is_some());
}

#[test]
fn view_node_on_click_attaches_state_capture_fingerprint() {
    let count = crate::ui::state::State::new(0);
    let node = label("tap").on_click(&count, |c| c.update(|v| *v += 1));
    assert_eq!(node.handlers.len(), 1);
    assert!(node.handlers[0].signature().capture_fingerprint.is_some());
}

#[test]
fn show_omits_child_when_false() {
    assert!(show(false, label("hidden")).is_empty());
    assert_eq!(show(true, label("visible")).len(), 1);
}

#[test]
fn option_view_builds_space_when_none() {
    let some = View::build(Some(label("ok")));
    assert!(some
        .widget
        .as_any()
        .downcast_ref::<crate::ui::widgets::Label>()
        .is_some());
    let none = View::build(None::<ViewNode>);
    assert!(none
        .widget
        .as_any()
        .downcast_ref::<crate::ui::widgets::Space>()
        .is_some());
}

#[test]
fn column_fit_keeps_intrinsic_flex_grow() {
    use crate::ui::traits::WidgetLayout;
    use crate::ui::widgets::Container;

    let grow = column([label("fill")]);
    let fit = column_fit([label("intrinsic")]);

    let grow_w = grow
        .widget
        .as_any()
        .downcast_ref::<Container>()
        .expect("column widget");
    let fit_w = fit
        .widget
        .as_any()
        .downcast_ref::<Container>()
        .expect("column_fit widget");

    assert_eq!(grow_w.flex_grow(), 1.0);
    assert_eq!(fit_w.flex_grow(), 0.0);
}

#[test]
fn with_cloned_captures_states_for_static_root() {
    let a = crate::ui::state::State::new(1i32);
    let b = crate::ui::state::State::new(2i32);
    let root = crate::with_cloned!(a, b; {
        column((
            a.map_text(|n| format!("{n}")),
            b.map_text(|n| format!("{n}")),
        ))
    });
    let node = root();
    assert_eq!(node.children.len(), 2);
    a.set(10);
    let node2 = root();
    assert_eq!(node2.children.len(), 2);
}

/// README Counter 的编译探针（[使用](docs/使用.md) · [README · 示例](../../../../../README.md#示例)）。
#[test]
fn counter_facade_compiles_without_manual_into_or_clone_aliases() {
    let count = crate::ui::state::State::new(0);
    let _view = column((
        count.map_text(|n| format!("当前值: {n}")).font_size(24.0),
        button("+1").primary().on_click(&count, |c| {
            c.update(|v| *v += 1);
        }),
    ))
    .gap(12.0)
    .padding(16.0);
}
