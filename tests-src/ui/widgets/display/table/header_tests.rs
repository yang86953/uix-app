//! `src/ui/widgets/display/table/header.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的自由 cfg(test) 项 ——

// 仅为全局分组片段测试保留跨列区访问器。
#[cfg(test)]
// 按全局列区层级访问可见分组表头片段且不分配中间集合。
fn visit_group_title_segments(
    // 提供分组声明及列配置。
    table: &Table,
    // 提供上层分组表头的实际矩形。
    group_rect: Rect,
    // 提供各列在当前视口中的共享几何。
    geometry: &TableColumnGeometry,
    // 接收标题、列区与裁剪后片段。
    mut visit: impl FnMut(&str, super::super::geometry::ColumnZone, Rect),
) {
    // 先按共享全局列区层级遍历，避免后声明的低层分组覆盖高层固定区。
    for zone in COLUMN_PAINT_ORDER {
        // 复用单列区访问器并补回调用方需要的列区信息。
        visit_group_title_segments_for_zone(
            // 传入当前表格声明。
            table,
            // 传入上层分组表头矩形。
            group_rect,
            // 传入共享列几何。
            geometry,
            // 限定当前全局列区。
            zone,
            // 将标题、列区与片段转发给调用方。
            |title, segment| visit(title, zone, segment),
        );
    }
}