// 复用分组片段访问器和父模块导入的几何类型。
use super::*;
// 引入固定列、表格列与分组声明类型。
use super::super::types::{Fixed, TableColumn, TableColumnGroup};

// 标记分组声明顺序不得覆盖全局列区层级的契约。
#[test]
// 验证右固定组即使声明在前也必须最后绘制。
fn overlapping_group_headers_follow_global_zone_paint_order() {
    // 构造声明在前且位于最上层的右固定分组。
    let right_group = TableColumnGroup::new(
        // 设置可识别的右组标题。
        "右组",
        // 放入一个宽于剩余视口的右固定列。
        vec![TableColumn::new("右列", 80.0).fixed(Fixed::Right)],
    );
    // 构造声明在后但视觉层级较低的左固定分组。
    let left_group = TableColumnGroup::new(
        // 设置可识别的左组标题。
        "左组",
        // 放入一个同样宽的左固定列。
        vec![TableColumn::new("左列", 80.0).fixed(Fixed::Left)],
    );
    // 按右组在前、左组在后的顺序构造表格。
    let table = Table::new().column_groups(vec![right_group, left_group]);
    // 在一百像素视口中形成二十到八十像素的固定区重叠。
    let geometry = table.column_geometry(0.0, 100.0);
    // 收集生产访问器产生的列区绘制顺序。
    let mut painted_zones = Vec::new();
    // 使用真实分组表头矩形访问所有可见片段。
    visit_group_title_segments(
        // 传入当前表格声明。
        &table,
        // 两层表头的上半层使用默认表头高度。
        Rect::new(0.0, 0.0, 100.0, table.header_h),
        // 传入与命中测试相同的列几何。
        &geometry,
        // 只记录每个片段所属列区。
        |_, zone, _| painted_zones.push(zone),
    );
    // 全局层级要求左固定区先绘制、右固定区最后覆盖。
    assert_eq!(
        painted_zones,
        vec![
            // 左固定分组应先进入绘制队列。
            super::super::geometry::ColumnZone::Left,
            // 右固定分组必须最后进入绘制队列。
            super::super::geometry::ColumnZone::Right,
        ]
    );
    // 重叠点命中仍以最后绘制的右固定列为准。
    assert_eq!(geometry.column_at(50.0), Some(0));
    // 结束分组表头全局列区层级契约。
}

// 标记跨两层单列必须服从全局列区层级的契约。
#[test]
// 验证较低层左固定单列不能覆盖右固定分组的上层标题。
fn spanning_leaf_header_cannot_cover_higher_fixed_group_title() {
    // 构造声明在前且位于最高列区的右固定分组。
    let right_group = TableColumnGroup::new(
        // 设置可识别的分组标题。
        "右组",
        // 放入一个八十像素右固定列。
        vec![TableColumn::new("右列", 80.0).fixed(Fixed::Right)],
    );
    // 构造声明在后且跨越两层的左固定单列。
    let left_column = TableColumnGroup::column(
        // 单列宽度同为八十像素，从而与右固定区重叠。
        TableColumn::new("左列", 80.0).fixed(Fixed::Left),
    );
    // 按右分组在前、左单列在后的顺序构造两层表头。
    let table = Table::new().column_groups(vec![right_group, left_column]);
    // 在一百像素视口中形成二十到八十像素的重叠区。
    let geometry = table.column_geometry(0.0, 100.0);
    // 构造覆盖两层的完整表头矩形。
    let header_rect = Rect::new(0.0, 0.0, 100.0, table.total_header_height());
    // 构造仅覆盖上层分组标题的矩形。
    let group_rect = Rect::new(0.0, 0.0, 100.0, table.header_h);
    // 记录经过重叠点的真实阶段片段访问顺序。
    let mut overlap_paints = Vec::new();
    // 使用生产绘制调用的共享阶段访问器。
    visit_header_zone_layers(
        // 以访问顺序集合承载两个阶段的共享状态。
        &mut overlap_paints,
        // 记录经过上层重叠点的分组标题片段。
        |paints, zone| {
            // 访问当前列区的真实分组片段几何。
            visit_group_title_segments_for_zone(
                // 传入当前表格声明。
                &table,
                // 传入上层分组矩形。
                group_rect,
                // 传入共享列几何。
                &geometry,
                // 限定当前阶段列区。
                zone,
                // 只记录覆盖横坐标五十、纵坐标十的标题片段。
                |title, segment| {
                    // 检查目标点是否位于当前分组片段中。
                    if 50.0 >= segment.x
                            // 排除右开边界之外的点。
                            && 50.0 < segment.x + segment.w
                            // 检查目标点位于片段顶部之后。
                            && 10.0 >= segment.y
                            // 排除底部右开边界之外的点。
                            && 10.0 < segment.y + segment.h
                    {
                        // 记录可识别的分组标题。
                        paints.push(format!("分组:{title}"));
                    }
                },
            );
        },
        // 记录经过同一上层重叠点的叶表头片段。
        |paints, zone| {
            // 按生产叶表头路径访问当前列区的已布局列。
            for laid_out in geometry.columns.iter().filter(|column| column.zone == zone) {
                // 判断当前列是否属于有标题分组。
                let grouped = table
                    // 访问全部分组范围。
                    .column_groups
                    // 转为稳定迭代器。
                    .iter()
                    // 查找覆盖当前扁平列索引的分组。
                    .find(|group| {
                        // 同时约束分组起点与末端。
                        laid_out.index >= group.start
                                // 继续检查当前索引位于分组末端之前。
                                && laid_out.index < group.start + group.len
                    })
                    // 只有有标题分组才使用下层叶矩形。
                    .is_some_and(|group| group.title.is_some());
                // 复现生产路径中的跨层单列与下层叶矩形选择。
                let leaf_rect = if grouped {
                    // 有标题分组的叶表头只占下半层。
                    Rect::new(
                        // 继承完整表头左边界。
                        header_rect.x,
                        // 从一层表头高度之后开始。
                        header_rect.y + table.header_h,
                        // 继承完整表头宽度。
                        header_rect.w,
                        // 叶层高度等于单层表头高度。
                        table.header_h,
                    )
                } else {
                    // 无标题单列跨越完整两层。
                    header_rect
                };
                // 检查当前列水平范围与叶矩形是否共同覆盖目标点。
                if 50.0 >= laid_out.x
                        // 排除列水平右开边界之外的点。
                        && 50.0 < laid_out.x + laid_out.width
                        // 检查目标点位于叶矩形顶部之后。
                        && 10.0 >= leaf_rect.y
                        // 排除叶矩形底部右开边界之外的点。
                        && 10.0 < leaf_rect.y + leaf_rect.h
                {
                    // 记录可识别的叶表头标题。
                    paints.push(format!("单列:{}", table.columns[laid_out.index].title));
                }
            }
        },
    );
    // 目标点应先经过低层左单列，最后由高层右分组标题覆盖。
    assert_eq!(
        // 比较经过目标点的完整片段顺序。
        overlap_paints,
        // 期望顺序与 Middle 到 Left 到 Right 的全局层级一致。
        vec!["单列:左列".to_owned(), "分组:右组".to_owned()]
    );
    // 同一点的列命中必须继续落在视觉最上层右固定列。
    assert_eq!(geometry.column_at(50.0), Some(0));
    // 结束跨两层单列表头层级契约。
}
// 结束分组表头测试模块。
