// 引入放置模块的私有常量、输出与辅助函数。
use super::*;
// 引入构造 Grid 子项所需的几何类型。
use crate::core::Size;

// 构造一个只覆盖放置字段的 Grid 子项。
fn grid_child(cell: Option<usize>, col_span: u32, row_span: u32) -> GridChild {
    // 从零尺寸默认值开始，避免尺寸求解干扰放置断言。
    GridChild {
        // 写入当前测试所需的显式或自动放置。
        cell,
        // 写入待归一的列 span。
        col_span,
        // 写入待归一的行 span。
        row_span,
        // 放置测试不需要内容尺寸贡献。
        measured_size: Size::zero(),
        // 其余字段沿用安全默认值。
        ..GridChild::default()
    }
}

// 使用直接遍历计算小矩阵中的第一个空闲矩形。
fn brute_force_available_rect(
    occupied: &[bool],
    column_count: usize,
    col_span: usize,
    row_span: usize,
) -> Option<(usize, usize)> {
    // 从线性占用账本恢复行数。
    let row_count = occupied.len() / column_count;
    // 按行优先顺序遍历所有合法起点。
    for row in 0..=row_count - row_span {
        // 在当前行中从左到右遍历候选列。
        for col in 0..=column_count - col_span {
            // 默认当前候选矩形完全空闲。
            let mut available = true;
            // 直接检查候选矩形覆盖的每一行。
            for occupied_row in row..row + row_span {
                // 直接检查候选矩形覆盖的每一列。
                for occupied_col in col..col + col_span {
                    // 任一单元格已占用即否决当前候选。
                    if occupied[occupied_row * column_count + occupied_col] {
                        // 记录候选矩形不可用。
                        available = false;
                    }
                }
            }
            // 第一个完全空闲的候选就是行优先答案。
            if available {
                // 返回与生产搜索相同的列、行顺序。
                return Some((col, row));
            }
        }
    }
    // 所有候选均被占用时返回空结果。
    None
}

// 验证列数与行数同时受单轴和总单元格预算约束。
#[test]
// 覆盖极大列请求及单列/最大列的行窗口。
fn placement_limits_bound_both_axes_and_total_cells() {
    // 极大列数只保留单轴上限。
    assert_eq!(bounded_column_count(usize::MAX), 4_096);
    // 单列网格可使用完整单轴行预算。
    assert_eq!(placement_row_limit(1), 4_096);
    // 四千九十六列时行窗口收窄到十六，总单元格恰好不超预算。
    assert_eq!(placement_row_limit(4_096), 16);
}

// 验证 usize 与 u32 极值不会触发索引溢出或过量分配。
#[test]
// 覆盖双列网格中的极大显式 cell 与行 span。
fn extreme_explicit_placement_clamps_to_the_last_cell() {
    // 构造同时使用两种整数极值的显式子项。
    let children = [grid_child(Some(usize::MAX), 1, u32::MAX)];
    // 在两列且没有显式行定义的 Grid 中执行放置。
    let plan = place_grid_children(2, 0, &children);
    // 极大 cell 最终只物化四千九十六行。
    assert_eq!(plan.row_count, 4_096);
    // 显式子项必须保留一个有界定位。
    assert_eq!(plan.assignments.len(), 1);
    // 读取唯一定位结果。
    let assignment = &plan.assignments[0];
    // 最后一个双列单元格位于次列。
    assert_eq!(assignment.col, 1);
    // 最后一个双列单元格位于第四千九十五行。
    assert_eq!(assignment.row, 4_095);
    // 起点之后只剩一行，极大 span 因此收敛为一。
    assert_eq!(assignment.row_span, 1);
}

// 验证自动子项的极大行 span 会收敛并且仅物化一个有界矩形。
#[test]
// 覆盖自动放置的 u32::MAX 行 span。
fn extreme_auto_span_uses_the_bounded_row_window() {
    // 构造没有显式 cell 的极大跨行子项。
    let children = [grid_child(None, 1, u32::MAX)];
    // 在单列 Grid 中执行自动放置。
    let plan = place_grid_children(1, 0, &children);
    // 自动 span 只能推高到单轴行上限。
    assert_eq!(plan.row_count, 4_096);
    // 子项在有界窗口中成功放置。
    assert_eq!(plan.assignments.len(), 1);
    // 放置结果保留完整有界 span。
    assert_eq!(plan.assignments[0].row_span, 4_096);
}

// 验证有界矩阵完全占满后自动放置会停止而不无限扩行。
#[test]
// 覆盖显式全高矩形阻塞后续自动子项的边界。
fn exhausted_placement_budget_leaves_auto_child_unassigned() {
    // 首项显式占满单列网格的整个有界行窗口。
    let explicit = grid_child(Some(0), 1, u32::MAX);
    // 次项请求自动放置的单格矩形。
    let automatic = grid_child(None, 1, 1);
    // 在单列 Grid 中执行两个子项的放置。
    let plan = place_grid_children(1, 0, &[explicit, automatic]);
    // 行数保持在单轴资源上限。
    assert_eq!(plan.row_count, 4_096);
    // 只有显式子项获得定位，自动子项由上层保留零 frame。
    assert_eq!(plan.assignments.len(), 1);
    // 唯一定位仍指向首个显式子项。
    assert_eq!(plan.assignments[0].child_idx, 0);
}

// 验证有界放置仍保留原有的行优先稠密搜索顺序。
#[test]
// 覆盖跨列自动子项扩行后，后续单格子项回填首行空位的路径。
fn bounded_auto_placement_remains_row_major_and_dense() {
    // 显式子项占用首行前两列。
    let explicit = grid_child(Some(0), 2, 1);
    // 第一个自动子项同样需要连续两列，因此必须扩到第二行。
    let wide_automatic = grid_child(None, 2, 1);
    // 第二个自动子项只需要一格，应回填首行第三列。
    let single_automatic = grid_child(None, 1, 1);
    // 在三列 Grid 中按输入顺序执行放置。
    let plan = place_grid_children(3, 0, &[explicit, wide_automatic, single_automatic]);
    // 只需要两行就能容纳全部子项。
    assert_eq!(plan.row_count, 2);
    // 三个子项都应成功获得定位。
    assert_eq!(plan.assignments.len(), 3);
    // 跨列自动子项放在第二行首列。
    assert_eq!((plan.assignments[1].col, plan.assignments[1].row), (0, 1));
    // 后续单格子项仍从首行搜索并回填第三列。
    assert_eq!((plan.assignments[2].col, plan.assignments[2].row), (2, 0));
}

// 验证二维前缀和搜索与直接遍历的空闲矩形语义完全一致。
#[test]
// 穷举三乘三矩阵的五百一十二种占用状态和全部 span。
fn prefix_search_matches_brute_force_for_every_small_grid() {
    // 前缀和缓冲在全部穷举用例之间复用。
    let mut prefix = Vec::new();
    // 三乘三 bool 矩阵共有二的九次方种占用状态。
    for mask in 0usize..(1usize << 9) {
        // 从位掩码构造当前线性占用账本。
        let occupied: Vec<bool> = (0..9).map(|index| mask & (1usize << index) != 0).collect();
        // 穷举一到三列的所有横向 span。
        for col_span in 1..=3 {
            // 穷举一到三行的所有纵向 span。
            for row_span in 1..=3 {
                // 使用直接遍历建立可读参考结果。
                let expected = brute_force_available_rect(&occupied, 3, col_span, row_span);
                // 使用生产前缀和搜索计算实际结果。
                let actual = find_available_rect(&occupied, 3, col_span, row_span, &mut prefix);
                // 每种占用状态与 span 都必须保持完全相同的行优先答案。
                assert_eq!(
                    actual, expected,
                    "mask={mask:#011b}, span={col_span}x{row_span}"
                );
            }
        }
    }
}
