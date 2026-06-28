//! uix-ui layout 模块集成测试。

use uix_ui::{compute_flex_layout, compute_grid_layout};
use uix_ui::layout::{
    AlignItems, FlexChild, FlexDirection, FlexInput, GridChild, GridInput, GridTrack, JustifyContent,
};
use uix_platform::geometry::{Rect, Size};

// ════════════════════════════════════════════════════════════════════════════
// flex 测试
// ════════════════════════════════════════════════════════════════════════════

fn make_flex_input(
    c: Rect,
    sizes: Vec<Size>,
    dir: FlexDirection,
    j: JustifyContent,
    a: AlignItems,
) -> FlexInput {
    FlexInput {
        direction: dir,
        container: c,
        children: vec![FlexChild::default(); sizes.len()],
        child_sizes: sizes,
        justify_content: j,
        align_items: a,
        ..FlexInput::default()
    }
}

#[test]
fn flex_row_start_top() {
    let o = compute_flex_layout(&make_flex_input(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::Start,
        AlignItems::Start,
    ));
    assert_eq!(o.child_rects[0].x, 0.0);
    assert_eq!(o.child_rects[0].y, 0.0);
    assert_eq!(o.child_rects[0].w, 80.0);
    assert_eq!(o.child_rects[0].h, 40.0);
    assert_eq!(o.child_rects[1].x, 80.0);
    assert_eq!(o.child_rects[1].y, 0.0);
}

#[test]
fn flex_row_center_top() {
    let o = compute_flex_layout(&make_flex_input(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::Center,
        AlignItems::Start,
    ));
    assert!((o.child_rects[0].x - 50.0).abs() < 0.001);
}

#[test]
fn flex_row_end_top() {
    let o = compute_flex_layout(&make_flex_input(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::End,
        AlignItems::Start,
    ));
    assert_eq!(o.child_rects[0].x, 100.0);
}

#[test]
fn flex_row_space_between() {
    let o = compute_flex_layout(&make_flex_input(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::SpaceBetween,
        AlignItems::Start,
    ));
    assert_eq!(o.child_rects[0].x, 0.0);
    assert_eq!(o.child_rects[1].x, 180.0);
}

#[test]
fn flex_column_start_top() {
    let o = compute_flex_layout(&make_flex_input(
        Rect::new(0.0, 0.0, 300.0, 300.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Column,
        JustifyContent::Start,
        AlignItems::Start,
    ));
    assert_eq!(o.child_rects[0].x, 0.0);
    assert_eq!(o.child_rects[0].y, 0.0);
    assert_eq!(o.child_rects[1].x, 0.0);
    assert_eq!(o.child_rects[1].y, 40.0);
}

#[test]
fn flex_column_space_between() {
    let o = compute_flex_layout(&make_flex_input(
        Rect::new(0.0, 0.0, 300.0, 300.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Column,
        JustifyContent::SpaceBetween,
        AlignItems::Start,
    ));
    assert_eq!(o.child_rects[0].y, 0.0);
    assert_eq!(o.child_rects[1].y, 240.0);
}

#[test]
fn flex_row_align_center() {
    let o = compute_flex_layout(&make_flex_input(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0)],
        FlexDirection::Row,
        JustifyContent::Start,
        AlignItems::Center,
    ));
    assert!((o.child_rects[0].y - 30.0).abs() < 0.001);
}

#[test]
fn flex_row_align_end() {
    let o = compute_flex_layout(&make_flex_input(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0)],
        FlexDirection::Row,
        JustifyContent::Start,
        AlignItems::End,
    ));
    assert!((o.child_rects[0].y - 60.0).abs() < 0.001);
}

#[test]
fn flex_row_stretch() {
    let o = compute_flex_layout(&make_flex_input(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::Start,
        AlignItems::Stretch,
    ));
    assert_eq!(o.child_rects[0].h, 100.0);
    assert_eq!(o.child_rects[1].h, 100.0);
}

#[test]
fn flex_row_space_around() {
    let o = compute_flex_layout(&make_flex_input(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::SpaceAround,
        AlignItems::Start,
    ));
    assert!(o.child_rects[0].x > 0.0);
    assert!(o.child_rects[1].x < 200.0);
}

#[test]
fn flex_row_space_evenly() {
    let o = compute_flex_layout(&make_flex_input(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::SpaceEvenly,
        AlignItems::Start,
    ));
    assert!(o.child_rects[0].x > 0.0);
    assert!(o.child_rects[1].x < 200.0);
}

#[test]
fn flex_empty_children() {
    let o = compute_flex_layout(&make_flex_input(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![],
        FlexDirection::Row,
        JustifyContent::Start,
        AlignItems::Start,
    ));
    assert!(o.child_rects.is_empty());
}

#[test]
fn flex_row_with_gap() {
    let o = compute_flex_layout(&FlexInput {
        direction: FlexDirection::Row,
        container: Rect::new(0.0, 0.0, 300.0, 100.0),
        children: vec![FlexChild::default(), FlexChild::default()],
        child_sizes: vec![Size::new(80.0, 40.0), Size::new(80.0, 40.0)],
        justify_content: JustifyContent::Start,
        align_items: AlignItems::Start,
        gap: 20.0,
        ..FlexInput::default()
    });
    assert_eq!(o.child_rects[0].x, 0.0);
    assert_eq!(o.child_rects[1].x, 100.0);
}

#[test]
fn flex_row_wrap_basic() {
    let o = compute_flex_layout(&FlexInput {
        direction: FlexDirection::Row,
        container: Rect::new(0.0, 0.0, 200.0, 200.0),
        children: vec![FlexChild::default(); 4],
        child_sizes: vec![Size::new(120.0, 50.0); 4],
        justify_content: JustifyContent::Start,
        align_items: AlignItems::Start,
        wrap: true,
        ..FlexInput::default()
    });
    // 每个子项 120px，容器 200px → 每行一个，共 4 行
    assert_eq!(o.child_rects.len(), 4);
    assert_eq!(o.child_rects[0].y, 0.0);
    assert_eq!(o.child_rects[1].y, 50.0);
    assert_eq!(o.child_rects[2].y, 100.0);
    assert_eq!(o.child_rects[3].y, 150.0);
}

// ════════════════════════════════════════════════════════════════════════════
// grid 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn grid_2x2_fixed() {
    let o = compute_grid_layout(&GridInput {
        container: Rect::new(0.0, 0.0, 200.0, 200.0),
        columns: vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
        rows: vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
        align_items: AlignItems::Stretch,
        justify_items: JustifyContent::Stretch,
        children: vec![GridChild { preferred_size: Size::new(50.0, 50.0), ..Default::default() }; 4],
        ..Default::default()
    });
    assert_eq!(o.child_rects[0], Rect::new(0.0, 0.0, 100.0, 100.0));
    assert_eq!(o.child_rects[1], Rect::new(100.0, 0.0, 100.0, 100.0));
    assert_eq!(o.child_rects[2], Rect::new(0.0, 100.0, 100.0, 100.0));
    assert_eq!(o.child_rects[3], Rect::new(100.0, 100.0, 100.0, 100.0));
}

#[test]
fn grid_fraction_columns() {
    let o = compute_grid_layout(&GridInput {
        container: Rect::new(0.0, 0.0, 300.0, 100.0),
        columns: vec![GridTrack::Fr(1.0), GridTrack::Fr(2.0)],
        rows: vec![GridTrack::Px(100.0)],
        align_items: AlignItems::Stretch,
        justify_items: JustifyContent::Stretch,
        children: vec![GridChild { preferred_size: Size::new(50.0, 50.0), ..Default::default() }; 2],
        ..Default::default()
    });
    assert!((o.child_rects[0].w - 100.0).abs() < 1.0);
    assert!((o.child_rects[1].w - 200.0).abs() < 1.0);
}

#[test]
fn grid_with_gap() {
    let o = compute_grid_layout(&GridInput {
        container: Rect::new(0.0, 0.0, 210.0, 210.0),
        columns: vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
        rows: vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
        col_gap: 10.0,
        row_gap: 10.0,
        align_items: AlignItems::Stretch,
        justify_items: JustifyContent::Stretch,
        children: vec![GridChild { preferred_size: Size::new(50.0, 50.0), ..Default::default() }],
        ..Default::default()
    });
    assert_eq!(o.child_rects[0], Rect::new(0.0, 0.0, 100.0, 100.0));
    assert!((o.col_positions[1].0 - 110.0).abs() < 1.0);
}

#[test]
fn grid_empty() {
    let o = compute_grid_layout(&GridInput::default());
    assert!(o.child_rects.is_empty());
}

#[test]
fn grid_fr_auto_mixed() {
    let o = compute_grid_layout(&GridInput {
        container: Rect::new(0.0, 0.0, 300.0, 100.0),
        columns: vec![GridTrack::Fr(1.0), GridTrack::Auto],
        rows: vec![GridTrack::Px(100.0)],
        align_items: AlignItems::Stretch,
        justify_items: JustifyContent::Stretch,
        children: vec![
            GridChild { preferred_size: Size::new(50.0, 50.0), ..Default::default() },
            GridChild { preferred_size: Size::new(50.0, 50.0), ..Default::default() },
        ],
        ..Default::default()
    });
    assert!((o.child_rects[0].w - 150.0).abs() < 1.0);
    assert!((o.child_rects[1].w - 150.0).abs() < 1.0);
}

#[test]
fn grid_implicit_rows() {
    let o = compute_grid_layout(&GridInput {
        container: Rect::new(0.0, 0.0, 200.0, 200.0),
        columns: vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
        rows: vec![GridTrack::Px(100.0)],
        align_items: AlignItems::Stretch,
        justify_items: JustifyContent::Stretch,
        children: vec![
            GridChild { cell: 0, ..Default::default() },
            GridChild { cell: 1, ..Default::default() },
            GridChild { cell: 2, ..Default::default() },
            GridChild { cell: 3, ..Default::default() },
        ],
        ..Default::default()
    });
    assert_eq!(o.child_rects.len(), 4);
    assert_eq!(o.child_rects[2].y, o.child_rects[0].h);
    assert_eq!(o.child_rects[3].y, o.child_rects[0].h);
}
