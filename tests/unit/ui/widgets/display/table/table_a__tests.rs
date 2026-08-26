// 引入指针坐标类型以构造表头与表体测试点。
use crate::core::{Point, Rect};
// 复用表格动作解析与父模块已导入的几何类型。
use super::*;

// 标记选择列被右固定列覆盖时的交互层级契约。
#[test]
// 验证可见右固定列优先于被遮挡的选择列响应指针。
fn right_fixed_column_over_selection_uses_topmost_visible_action() {
    // 构造会侵入三十二像素选择列的宽右固定列。
    let right = TableColumn::new("右列", 90.0)
        // 启用表头排序以暴露列动作。
        .sortable(true)
        // 将该列固定到视口右侧。
        .fixed(super::super::types::Fixed::Right);
    // 在一百像素视口中开启选择列并保留一行数据。
    let table = Table::new()
        // 安装覆盖选择区十到三十二像素范围的右固定列。
        .columns(vec![right])
        // 添加一行以启用表头全选与表体行选择动作。
        .rows(vec![vec!["值".to_string()]])
        // 开启三十二像素选择列。
        .selection(true)
        // 固定窄视口尺寸以形成覆盖关系。
        .size(100.0, 100.0);
    // 模拟渲染阶段记录的实际表格 frame，确保动作几何使用真实视口。
    table
        // 写入与声明尺寸一致的运行态 frame。
        .last_frame
        // 让后续列几何按一百像素视口解析。
        .set(Some(Rect::new(0.0, 0.0, 100.0, 100.0)));
    // 表头重叠点显示右列时必须触发右列排序。
    assert_eq!(
        table.action_at_point(Point::new(20.0, 10.0)),
        Some(TablePointerAction::SortColumn(0))
    );
    // 计算第一行表体中的重叠测试纵坐标。
    let body_y = table.total_header_height() + 2.0;
    // 表体重叠点显示右列时必须执行普通行选择而非复选框切换。
    assert_eq!(
        table.action_at_point(Point::new(20.0, body_y)),
        Some(TablePointerAction::SelectRow(0))
    );
    // 选择区未被列覆盖的左端仍保留复选框动作。
    assert_eq!(
        table.action_at_point(Point::new(5.0, 10.0)),
        Some(TablePointerAction::ToggleAll)
    );
    // 结束选择列重叠交互契约。
}

// 标记极窄表体中展开箭头覆盖选择复选框时的交互层级契约。
#[test]
// 验证最后绘制的展开箭头优先于被遮挡的行复选框响应指针。
fn expand_toggle_over_selection_uses_topmost_visible_action() {
    // 构造带普通数据列和一行数据的可选择表格。
    let mut table = Table::new()
        // 安装从选择列右侧开始布局的普通列。
        .columns(vec![TableColumn::new("数据", 80.0)])
        // 添加一行以启用表体动作。
        .rows(vec![vec!["值".to_string()]])
        // 开启三十二像素选择列。
        .selection(true)
        // 把视口压窄到展开箭头与选择复选框重叠。
        .size(24.0, 100.0);
    // 启用物理行末尾最后绘制的展开箭头。
    table.expandable = true;
    // 模拟渲染阶段记录的实际极窄表格 frame。
    table
        // 写入与声明尺寸一致的运行态 frame。
        .last_frame
        // 让动作几何按二十四像素视口解析。
        .set(Some(Rect::new(0.0, 0.0, 24.0, 100.0)));
    // 表头没有展开箭头，仍应响应可见的全选复选框。
    assert_eq!(
        table.action_at_point(Point::new(10.0, 10.0)),
        Some(TablePointerAction::ToggleAll)
    );
    // 计算第一行表体中的重叠测试纵坐标。
    let body_y = table.total_header_height() + 2.0;
    // 表体同一点显示展开箭头，必须切换展开而不是勾选行。
    assert_eq!(
        table.action_at_point(Point::new(10.0, body_y)),
        Some(TablePointerAction::ToggleExpand(0))
    );
    // 结束展开箭头覆盖选择列的交互契约。
}
// 结束表格选择列重叠测试模块。
