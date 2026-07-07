use super::*;
use crate::core::{Constraints, Size};
use crate::ui::traits::WidgetLayout;
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
fn dynamic_label_measure_clamps_current_text() {
    let label = DynamicLabel::new(|| "abcdef".to_string());

    let measured = label.measure(Constraints::loose(Size::new(30.0, 12.0)));

    assert_eq!(measured, Size::new(30.0, 12.0));
}
