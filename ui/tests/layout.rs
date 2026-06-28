//! uix-ui layout 模块集成测试。
//!
//! 使用公共 LayoutEngine API（FlexLayout / GridLayout）而非内部 compute_* 函数。

use uix_platform::geometry::{Rect, Size};
use uix_ui::layout::engine::{FlexLayout, GridLayout, LayoutChild, LayoutEngine};
use uix_ui::layout::{AlignItems, FlexDirection, GridTrack, JustifyContent};

/// 辅助：用 FlexLayout 执行布局并返回其 positions。
fn flex_layout(
    container: Rect,
    sizes: Vec<Size>,
    dir: FlexDirection,
    justify: JustifyContent,
    align: AlignItems,
) -> Vec<Rect> {
    let engine = FlexLayout {
        direction: dir,
        gap: 0.0,
        justify,
        align,
        wrap: false,
        overflow_content: false,
    };
    let children: Vec<LayoutChild> = sizes
        .into_iter()
        .enumerate()
        .map(|(i, s)| LayoutChild::new(i, s))
        .collect();
    engine.layout(container, &children).positions
}

/// 辅助：用 GridLayout 执行布局并返回其 positions。
fn grid_layout(
    container: Rect,
    columns: Vec<GridTrack>,
    rows: Vec<GridTrack>,
    children: Vec<LayoutChild>,
) -> Vec<Rect> {
    let engine = GridLayout {
        columns,
        rows,
        col_gap: 0.0,
        row_gap: 0.0,
        align_items: AlignItems::Stretch,
        justify_items: JustifyContent::Stretch,
    };
    engine.layout(container, &children).positions
}

// ════════════════════════════════════════════════════════════════════════════
// flex 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn flex_row_start_top() {
    let positions = flex_layout(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::Start,
        AlignItems::Start,
    );
    assert_eq!(positions[0].x, 0.0);
    assert_eq!(positions[0].y, 0.0);
    assert_eq!(positions[0].w, 80.0);
    assert_eq!(positions[0].h, 40.0);
    assert_eq!(positions[1].x, 80.0);
    assert_eq!(positions[1].y, 0.0);
}

#[test]
fn flex_row_center_top() {
    let positions = flex_layout(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::Center,
        AlignItems::Start,
    );
    assert!((positions[0].x - 50.0).abs() < 0.001);
}

#[test]
fn flex_row_end_top() {
    let positions = flex_layout(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::End,
        AlignItems::Start,
    );
    assert_eq!(positions[0].x, 100.0);
}

#[test]
fn flex_row_space_between() {
    let positions = flex_layout(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::SpaceBetween,
        AlignItems::Start,
    );
    assert_eq!(positions[0].x, 0.0);
    assert_eq!(positions[1].x, 180.0);
}

#[test]
fn flex_column_start_top() {
    let positions = flex_layout(
        Rect::new(0.0, 0.0, 300.0, 300.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Column,
        JustifyContent::Start,
        AlignItems::Start,
    );
    assert_eq!(positions[0].x, 0.0);
    assert_eq!(positions[0].y, 0.0);
    assert_eq!(positions[1].x, 0.0);
    assert_eq!(positions[1].y, 40.0);
}

#[test]
fn flex_column_space_between() {
    let positions = flex_layout(
        Rect::new(0.0, 0.0, 300.0, 300.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Column,
        JustifyContent::SpaceBetween,
        AlignItems::Start,
    );
    assert_eq!(positions[0].y, 0.0);
    assert_eq!(positions[1].y, 240.0);
}

#[test]
fn flex_row_align_center() {
    let positions = flex_layout(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0)],
        FlexDirection::Row,
        JustifyContent::Start,
        AlignItems::Center,
    );
    assert!((positions[0].y - 30.0).abs() < 0.001);
}

#[test]
fn flex_row_align_end() {
    let positions = flex_layout(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0)],
        FlexDirection::Row,
        JustifyContent::Start,
        AlignItems::End,
    );
    assert!((positions[0].y - 60.0).abs() < 0.001);
}

#[test]
fn flex_row_stretch() {
    let positions = flex_layout(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::Start,
        AlignItems::Stretch,
    );
    assert_eq!(positions[0].h, 100.0);
    assert_eq!(positions[1].h, 100.0);
}

#[test]
fn flex_row_space_around() {
    let positions = flex_layout(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::SpaceAround,
        AlignItems::Start,
    );
    assert!(positions[0].x > 0.0);
    assert!(positions[1].x < 200.0);
}

#[test]
fn flex_row_space_evenly() {
    let positions = flex_layout(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
        FlexDirection::Row,
        JustifyContent::SpaceEvenly,
        AlignItems::Start,
    );
    assert!(positions[0].x > 0.0);
    assert!(positions[1].x < 200.0);
}

#[test]
fn flex_empty_children() {
    let positions = flex_layout(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![],
        FlexDirection::Row,
        JustifyContent::Start,
        AlignItems::Start,
    );
    assert!(positions.is_empty());
}

#[test]
fn flex_row_with_gap() {
    let engine = FlexLayout {
        direction: FlexDirection::Row,
        gap: 20.0,
        justify: JustifyContent::Start,
        align: AlignItems::Start,
        wrap: false,
        overflow_content: false,
    };
    let children = vec![
        LayoutChild::new(0, Size::new(80.0, 40.0)),
        LayoutChild::new(1, Size::new(80.0, 40.0)),
    ];
    let positions = engine
        .layout(Rect::new(0.0, 0.0, 300.0, 100.0), &children)
        .positions;
    assert_eq!(positions[0].x, 0.0);
    assert_eq!(positions[1].x, 100.0);
}

#[test]
fn flex_row_wrap_basic() {
    let engine = FlexLayout {
        direction: FlexDirection::Row,
        gap: 0.0,
        justify: JustifyContent::Start,
        align: AlignItems::Start,
        wrap: true,
        overflow_content: false,
    };
    let children: Vec<LayoutChild> = (0..4)
        .map(|i| LayoutChild::new(i, Size::new(120.0, 50.0)))
        .collect();
    let positions = engine
        .layout(Rect::new(0.0, 0.0, 200.0, 200.0), &children)
        .positions;
    // 每个子项 120px，容器 200px → 每行一个，共 4 行
    assert_eq!(positions.len(), 4);
    assert_eq!(positions[0].y, 0.0);
    assert_eq!(positions[1].y, 50.0);
    assert_eq!(positions[2].y, 100.0);
    assert_eq!(positions[3].y, 150.0);
}

// ════════════════════════════════════════════════════════════════════════════
// grid 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn grid_2x2_fixed() {
    let children: Vec<LayoutChild> = (0..4)
        .map(|i| LayoutChild::new(i, Size::new(50.0, 50.0)))
        .collect();
    let positions = grid_layout(
        Rect::new(0.0, 0.0, 200.0, 200.0),
        vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
        vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
        children,
    );
    assert_eq!(positions[0], Rect::new(0.0, 0.0, 100.0, 100.0));
    assert_eq!(positions[1], Rect::new(100.0, 0.0, 100.0, 100.0));
    assert_eq!(positions[2], Rect::new(0.0, 100.0, 100.0, 100.0));
    assert_eq!(positions[3], Rect::new(100.0, 100.0, 100.0, 100.0));
}

#[test]
fn grid_fraction_columns() {
    let children: Vec<LayoutChild> = (0..2)
        .map(|i| LayoutChild::new(i, Size::new(50.0, 50.0)))
        .collect();
    let positions = grid_layout(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![GridTrack::Fr(1.0), GridTrack::Fr(2.0)],
        vec![GridTrack::Px(100.0)],
        children,
    );
    assert!((positions[0].w - 100.0).abs() < 1.0);
    assert!((positions[1].w - 200.0).abs() < 1.0);
}

#[test]
fn grid_with_gap() {
    let children = vec![LayoutChild::new(0, Size::new(50.0, 50.0))];
    let engine = GridLayout {
        columns: vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
        rows: vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
        col_gap: 10.0,
        row_gap: 10.0,
        align_items: AlignItems::Stretch,
        justify_items: JustifyContent::Stretch,
    };
    let positions = engine
        .layout(Rect::new(0.0, 0.0, 210.0, 210.0), &children)
        .positions;
    assert_eq!(positions[0], Rect::new(0.0, 0.0, 100.0, 100.0));
}

#[test]
fn grid_empty() {
    let children: Vec<LayoutChild> = vec![];
    let positions = grid_layout(
        Rect::new(0.0, 0.0, 200.0, 200.0),
        vec![GridTrack::Px(100.0)],
        vec![GridTrack::Px(100.0)],
        children,
    );
    assert!(positions.is_empty());
}

#[test]
fn grid_fr_auto_mixed() {
    let children: Vec<LayoutChild> = (0..2)
        .map(|i| LayoutChild::new(i, Size::new(50.0, 50.0)))
        .collect();
    let positions = grid_layout(
        Rect::new(0.0, 0.0, 300.0, 100.0),
        vec![GridTrack::Fr(1.0), GridTrack::Auto],
        vec![GridTrack::Px(100.0)],
        children,
    );
    assert!((positions[0].w - 150.0).abs() < 1.0);
    assert!((positions[1].w - 150.0).abs() < 1.0);
}
