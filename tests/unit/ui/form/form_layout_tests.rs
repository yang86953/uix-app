// 复用表单组件、布局枚举与私有几何契约。
use super::*;
// 引入布局能力与树级复用工作区。
use crate::ui::{LayoutEngineScratch, WidgetLayout};

// 比较拥有型与复用型测量结果的稳定字段。
fn assert_measured_matches(actual: &[LayoutChild], expected: &[LayoutChild]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.measured_size, expected.measured_size);
        assert_eq!(actual.margin, expected.margin);
    }
}

// 验证表单容器的三种布局均保持拥有型与复用型几何一致。
#[test]
fn form_reusing_paths_match_owned_for_all_layouts() {
    let tree = WidgetTree::new();
    let frame = Rect::new(10.0, 20.0, 360.0, 180.0);
    let ids = [WidgetId::new(1), WidgetId::new(2), WidgetId::new(3)];
    for layout in [
        FormLayout::Inline,
        FormLayout::Horizontal,
        FormLayout::Vertical,
    ] {
        let form = Form::new().layout(layout);
        let expected_measured = form.measure_children(frame, &ids, &tree);
        let mut actual_measured = Vec::new();
        form.measure_children_into(frame, &ids, &tree, &mut actual_measured);
        assert_measured_matches(&actual_measured, &expected_measured);

        let expected = form.layout_children(frame, &expected_measured, &tree);
        let mut scratch = LayoutEngineScratch::default();
        let mut actual = Vec::new();
        form.layout_children_into(frame, &actual_measured, &tree, &mut scratch, &mut actual);
        assert_eq!(actual, expected);
    }
}

// 验证表单项内容区的三种布局均保持拥有型与复用型几何一致。
#[test]
fn form_item_reusing_paths_match_owned_for_all_layouts() {
    let tree = WidgetTree::new();
    let frame = Rect::new(5.0, 7.0, 320.0, 64.0);
    let ids = [WidgetId::new(1), WidgetId::new(2)];
    for layout in [
        FormLayout::Inline,
        FormLayout::Horizontal,
        FormLayout::Vertical,
    ] {
        let item = FormItem::new("字段").layout(layout);
        let expected_measured = item.measure_children(frame, &ids, &tree);
        let mut actual_measured = Vec::new();
        item.measure_children_into(frame, &ids, &tree, &mut actual_measured);
        assert_measured_matches(&actual_measured, &expected_measured);

        let expected = item.layout_children(frame, &expected_measured, &tree);
        let mut scratch = LayoutEngineScratch::default();
        let mut actual = Vec::new();
        item.layout_children_into(frame, &actual_measured, &tree, &mut scratch, &mut actual);
        assert_eq!(actual, expected);
    }
}
