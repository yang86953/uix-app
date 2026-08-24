use super::{Transfer, TransferItem, TransferPane};
use crate::core::Rect;
use crate::ui::{LayoutChild, LayoutEngineScratch, WidgetId, WidgetLayout, WidgetTree};

// 比较拥有型与复用型测量结果的稳定布局字段。
fn assert_measured_matches(actual: &[LayoutChild], expected: &[LayoutChild]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.measured_size, expected.measured_size);
        assert_eq!(actual.margin, expected.margin);
    }
}

// 验证无中间集合的筛选保持既有 Unicode 小写匹配语义与声明顺序。
#[test]
fn transfer_visible_indices_match_normalized_reference() {
    let mut transfer = Transfer::new().source(vec![
        TransferItem::new("alpha", "Alpha"),
        TransferItem::new("umlaut", "Äpfel"),
        TransferItem::new("ocean", "海洋"),
        TransferItem::new("beta", "beta"),
        TransferItem::new("kelvin", "Kelvin"),
    ]);

    for query in ["", "ALP", "ÄP", "洋", "BETA", "KEL"] {
        transfer.search_query = query.to_owned();
        let normalized_query = transfer.search_query.trim().to_lowercase();
        let expected: Vec<_> = transfer
            .source
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                normalized_query.is_empty()
                    || item.title.to_lowercase().contains(&normalized_query)
                    || item.key.to_lowercase().contains(&normalized_query)
            })
            .map(|(index, _)| index)
            .collect();
        let query = transfer.normalized_query();
        let actual: Vec<_> = transfer
            .visible_indices(TransferPane::Source, query.as_ref())
            .collect();

        assert_eq!(actual, expected, "搜索词 {query:?} 的结果必须保持兼容");
    }
}

// 验证复用路径保持筛选后的左右栏几何和默认测量描述符。
#[test]
fn transfer_reusing_paths_match_owned_filtered_geometry() {
    let mut transfer = Transfer::new()
        .source(vec![
            TransferItem::new("alpha", "Alpha"),
            TransferItem::new("beta", "Beta"),
            TransferItem::new("gamma", "Gamma"),
        ])
        .target(vec![
            TransferItem::new("target-beta", "目标 Beta"),
            TransferItem::new("target-delta", "目标 Delta"),
        ])
        .searchable(true);
    transfer.search_query = "BETA".to_owned();
    let tree = WidgetTree::new();
    let frame = Rect::new(10.0, 20.0, 360.0, 240.0);
    let child_ids: Vec<_> = (1..=5).map(WidgetId::new).collect();
    let expected_measured = transfer.measure_children(frame, &child_ids, &tree);
    let mut reused_measured = Vec::new();
    transfer.measure_children_into(frame, &child_ids, &tree, &mut reused_measured);
    assert_measured_matches(&reused_measured, &expected_measured);

    let expected = transfer.layout_children(frame, &expected_measured, &tree);
    let mut scratch = LayoutEngineScratch::default();
    let mut actual = Vec::new();
    transfer.layout_children_into(frame, &reused_measured, &tree, &mut scratch, &mut actual);
    assert_eq!(actual, expected);

    let layout = transfer.visual.layout;
    let half = ((frame.w - layout.button_column_width) * 0.5).max(layout.min_pane_half_width);
    let y = frame.y + layout.header_height + layout.search_height;
    assert_eq!(actual[0].1, Rect::zero());
    assert_eq!(actual[1].1, Rect::new(frame.x, y, half, layout.row_height));
    assert_eq!(actual[2].1, Rect::zero());
    assert_eq!(
        actual[3].1,
        Rect::new(
            frame.x + half + layout.button_column_width,
            y,
            half,
            layout.row_height,
        )
    );
    assert_eq!(actual[4].1, Rect::zero());
}
