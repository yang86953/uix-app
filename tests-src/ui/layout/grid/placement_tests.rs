//! `src/ui/layout/grid/placement.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的自由 cfg(test) 项 ——

// 保存有界放置阶段的完整结果。
#[cfg(test)]
pub(super) struct PlacementPlan {
    // 记录真正用于后续求解的行数。
    pub(super) row_count: usize,
    // 保持所有成功放置子项的定位账本。
    pub(super) assignments: Vec<CellAssignment>,
}

// 在统一轨道与单元格预算内完成显式和自动放置。
#[cfg(test)]
pub(super) fn place_grid_children(
    column_count: usize,
    explicit_row_count: usize,
    children: &[GridChild],
) -> PlacementPlan {
    let mut occupied = Vec::new();
    let mut assignments = Vec::new();
    let mut prefix = Vec::new();
    let row_count = place_grid_children_into(
        column_count,
        explicit_row_count,
        children,
        &mut occupied,
        &mut assignments,
        &mut prefix,
    );
    PlacementPlan {
        row_count,
        assignments,
    }
}