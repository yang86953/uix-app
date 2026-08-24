// 复用标签组件的私有布局与缓存契约。
use super::*;
// 引入树级布局工作区与布局能力。
use crate::ui::{LayoutEngineScratch, WidgetLayout};

// 验证四种标签栏方向均保持拥有型与复用型面板几何一致。
#[test]
fn reusing_layout_matches_owned_for_all_positions() {
    let frame = Rect::new(10.0, 20.0, 360.0, 220.0);
    let tree = WidgetTree::new();
    let child_ids = [WidgetId::new(2)];
    for position in [
        TabPosition::Top,
        TabPosition::Bottom,
        TabPosition::Left,
        TabPosition::Right,
    ] {
        let tabs = Tabs::new()
            .tab("概览", "overview")
            .tab("详情", "details")
            .active(1)
            .position(position);
        let expected_measured = tabs.measure_children(frame, &child_ids, &tree);
        let mut actual_measured = Vec::new();
        tabs.measure_children_into(frame, &child_ids, &tree, &mut actual_measured);
        assert_eq!(actual_measured.len(), expected_measured.len());
        for (actual, expected) in actual_measured.iter().zip(&expected_measured) {
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.measured_size, expected.measured_size);
            assert_eq!(actual.margin, expected.margin);
        }

        let expected = tabs.layout_children(frame, &expected_measured, &tree);
        let mut scratch = LayoutEngineScratch::default();
        let mut actual = Vec::new();

        tabs.layout_children_into(frame, &actual_measured, &tree, &mut scratch, &mut actual);

        assert_eq!(actual, expected);
        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0].0, child_ids[0]);
    }
}

// 验证四种方向重建标签区间时均保留第一次预热得到的数组申请。
#[test]
fn tab_range_rebuild_reuses_existing_allocation_for_all_positions() {
    for position in [
        TabPosition::Top,
        TabPosition::Bottom,
        TabPosition::Left,
        TabPosition::Right,
    ] {
        let tabs = Tabs::new()
            .tab("概览", "overview")
            .tab("详情", "details")
            .tab("历史", "history")
            .position(position);
        let first_extent = tabs.rebuild_tab_main_ranges(|label| label.chars().count() as f32 * 8.0);
        let ranges = tabs.tab_main_ranges.borrow();
        let allocation = ranges.as_ptr();
        let capacity = ranges.capacity();
        let expected = ranges.clone();
        drop(ranges);

        let second_extent =
            tabs.rebuild_tab_main_ranges(|label| label.chars().count() as f32 * 8.0);
        let ranges = tabs.tab_main_ranges.borrow();

        assert_eq!(ranges.as_ptr(), allocation);
        assert_eq!(ranges.capacity(), capacity);
        assert_eq!(*ranges, expected);
        assert_eq!(second_extent, first_extent);
    }
}
