// 引入 Grid 纯求解器的子项输入类型。
use super::GridChild;

// 单轴最多物化四千九十六条 Grid 轨道。
const MAX_GRID_TRACKS_PER_AXIS: usize = 4_096;
// 稠密占用账本最多物化六万五千五百三十六个单元格。
const MAX_GRID_PLACEMENT_CELLS: usize = 65_536;

// 保存单个子项经资源边界收敛后的网格位置。
pub(super) struct CellAssignment {
    // 指向原始子项切片的索引。
    pub(super) child_idx: usize,
    // 子项的起始列。
    pub(super) col: usize,
    // 子项的起始行。
    pub(super) row: usize,
    // 子项经列边界收敛后的 span。
    pub(super) col_span: u32,
    // 子项经行边界收敛后的 span。
    pub(super) row_span: u32,
}

// 保存有界放置阶段的完整结果。
#[cfg(test)]
pub(super) struct PlacementPlan {
    // 记录真正用于后续求解的行数。
    pub(super) row_count: usize,
    // 保持所有成功放置子项的定位账本。
    pub(super) assignments: Vec<CellAssignment>,
}

// 把外部列定义收敛到单轴轨道资源上限。
pub(super) fn bounded_column_count(requested: usize) -> usize {
    // 空列保持为空，非空列最多保留单轴预算。
    requested.min(MAX_GRID_TRACKS_PER_AXIS)
}

// 按列数同时落实单轴和总单元格预算。
fn placement_row_limit(column_count: usize) -> usize {
    // 调用方只能在至少有一列时进入放置阶段。
    debug_assert!(column_count > 0);
    // 列数越多，可物化行数越少，确保乘积不超过单元格预算。
    (MAX_GRID_PLACEMENT_CELLS / column_count).clamp(1, MAX_GRID_TRACKS_PER_AXIS)
}

// 将外部 span 收敛到当前起点之后的可用轨道。
fn bounded_span(requested: u32, available: usize) -> usize {
    // 放置起点保证至少剩一条轨道。
    debug_assert!(available > 0);
    // 零 span 按既有语义归一，超大 span 则停在资源窗口边界。
    (requested as usize).max(1).min(available)
}

// 在有界稠密账本中登记一个已归一的占用矩形。
fn mark_occupied(
    occupied: &mut [bool],
    column_count: usize,
    col: usize,
    row: usize,
    col_span: usize,
    row_span: usize,
) {
    // 逐行标记该矩形，不构造额外临时集合。
    for occupied_row in row..row + row_span {
        // 计算当前行中连续列区间的线性起点。
        let start = occupied_row * column_count + col;
        // 矩形已收敛到行边界，因此结束索引可直接加 span。
        let end = start + col_span;
        // 一次标记当前行中的全部占用单元格。
        occupied[start..end].fill(true);
    }
}

// 使用二维前缀和在有界矩阵中搜索首个完整空闲矩形。
fn find_available_rect(
    occupied: &[bool],
    column_count: usize,
    col_span: usize,
    row_span: usize,
    prefix: &mut Vec<usize>,
) -> Option<(usize, usize)> {
    // 占用账本始终以完整行物化。
    let row_count = occupied.len() / column_count;
    // 当前矩阵尚不足以容纳请求 span 时直接返回。
    if col_span > column_count || row_span > row_count {
        // 调用方可在资源预算内扩行后再搜索。
        return None;
    }
    // 前缀和每行和每列各保留一个零边界。
    let stride = column_count + 1;
    // 列数已受单轴预算限制，边界加一不会溢出。
    let prefix_rows = row_count + 1;
    // 使用检查乘法防守未来资源常量调整。
    let prefix_len = prefix_rows.checked_mul(stride)?;
    // 清除上一个子项搜索留下的前缀和。
    prefix.clear();
    // 复用同一缓冲并以零初始化当前矩阵。
    prefix.resize(prefix_len, 0);

    // 按行建立二维占用数量前缀和。
    for row in 0..row_count {
        // 累计当前行到当前列的占用数。
        let mut row_total = 0usize;
        // 逐列把当前行累计值与上方前缀值合并。
        for col in 0..column_count {
            // bool 占用标记转换为零或一的计数。
            row_total += usize::from(occupied[row * column_count + col]);
            // 读取同列上方矩形的累计占用数。
            let above = prefix[row * stride + col + 1];
            // 写入包含当前单元格的前缀和。
            prefix[(row + 1) * stride + col + 1] = above + row_total;
        }
    }

    // 按行优先顺序搜索与既有 Grid 自动放置一致的首个空闲矩形。
    for row in 0..=row_count - row_span {
        // 在当前行中从左到右检查所有可容纳 span 的起点。
        for col in 0..=column_count - col_span {
            // 计算候选矩形的排他结束行。
            let row_end = row + row_span;
            // 计算候选矩形的排他结束列。
            let col_end = col + col_span;
            // 读取候选矩形右下角的前缀和。
            let bottom_right = prefix[row_end * stride + col_end];
            // 读取候选矩形左上角之前的前缀和。
            let top_left = prefix[row * stride + col];
            // 读取候选矩形右上边界的前缀和。
            let top_right = prefix[row * stride + col_end];
            // 读取候选矩形左下边界的前缀和。
            let bottom_left = prefix[row_end * stride + col];
            // 候选矩形占用数为零时四角前缀和的两侧恰好相等。
            if bottom_right + top_left == top_right + bottom_left {
                // 返回第一个符合行优先顺序的空闲起点。
                return Some((col, row));
            }
        }
    }

    // 当前已物化行中没有完整空闲矩形。
    None
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

// 在调用方拥有的工作区内完成放置，避免布局收敛轮次重复申请账本。
pub(super) fn place_grid_children_into(
    column_count: usize,
    explicit_row_count: usize,
    children: &[GridChild],
    occupied: &mut Vec<bool>,
    assignments: &mut Vec<CellAssignment>,
    prefix: &mut Vec<usize>,
) -> usize {
    // 调用方已在空列快速返回后才进入放置阶段。
    debug_assert!(column_count > 0);
    // 根据当前列数计算同时满足两类预算的行数上限。
    let row_limit = placement_row_limit(column_count);
    // 显式行定义至少物化一行，并停在行预算边界。
    let initial_rows = explicit_row_count.max(1).min(row_limit);
    // 列数与行数已由总单元格预算约束，乘积可安全物化。
    let initial_cells = column_count * initial_rows;
    // 仅为当前必要行建立稠密 bool 占用账本。
    occupied.clear();
    occupied.resize(initial_cells, false);
    // 输出定位数不可能超过输入子项数，容量由树级工作区跨轮次保留。
    assignments.clear();
    assignments.reserve(children.len());
    // 最终轨道数只记录显式行和真正放置成功的子项。
    let mut used_rows = initial_rows;
    // 前缀和缓冲在所有自动子项之间复用。
    prefix.clear();
    // 线性 cell 索引只能落在有界行列矩阵中。
    let cell_capacity = column_count * row_limit;

    // 先登记全部显式 cell，保留显式子项允许重叠的既有语义。
    for (child_idx, child) in children.iter().enumerate() {
        // 自动子项留到显式占用全部登记后再放置。
        let Some(requested_cell) = child.cell else {
            // 跳过当前自动子项。
            continue;
        };
        // 超大 cell 确定性收敛到最后一个可地址单元格。
        let cell = requested_cell.min(cell_capacity - 1);
        // 从归一后的线性索引解析列位置。
        let col = cell % column_count;
        // 从归一后的线性索引解析行位置。
        let row = cell / column_count;
        // 列 span 不得越过当前行的右边界。
        let col_span = bounded_span(child.col_span, column_count - col);
        // 行 span 不得越过轨道资源窗口底部。
        let row_span = bounded_span(child.row_span, row_limit - row);
        // 收敛后的起点和 span 之和必然不超过行上限。
        let needed_rows = row + row_span;
        // 当前稠密账本不足时只扩展到该显式矩形的底部。
        if needed_rows > occupied.len() / column_count {
            // 扩容乘积已受全局单元格预算保护。
            occupied.resize(column_count * needed_rows, false);
        }
        // 将收敛后的显式矩形写入占用账本。
        mark_occupied(occupied, column_count, col, row, col_span, row_span);
        // 显式子项可以把最终行数推高到自身底部。
        used_rows = used_rows.max(needed_rows);
        // 保存与资源契约一致的最终定位。
        assignments.push(CellAssignment {
            // 保留原始子项顺序索引。
            child_idx,
            // 写入有界列起点。
            col,
            // 写入有界行起点。
            row,
            // 最多四千九十六条轨道可无损转为 u32。
            col_span: col_span as u32,
            // 最多四千九十六条轨道可无损转为 u32。
            row_span: row_span as u32,
        });
    }

    // 再为自动子项搜索完整空闲矩形。
    for (child_idx, child) in children.iter().enumerate() {
        // 显式子项已在上一阶段完成放置。
        if child.cell.is_some() {
            // 跳过当前显式子项。
            continue;
        }
        // 自动子项的列 span 只能占用当前有界列数。
        let col_span = bounded_span(child.col_span, column_count);
        // 自动子项的行 span 只能占用当前行资源窗口。
        let row_span = bounded_span(child.row_span, row_limit);
        // 读取当前已物化行数。
        let current_rows = occupied.len() / column_count;
        // 至少先物化能容纳当前 row span 的行数。
        if row_span > current_rows {
            // row span 已被行上限收敛，因此此处扩容不会越界。
            occupied.resize(column_count * row_span, false);
        }
        // 先在当前已物化行中按行优先顺序搜索。
        let mut placement = find_available_rect(occupied, column_count, col_span, row_span, prefix);
        // 当前矩阵无空闲矩形时，在预算内至多扩展一个 span 的行数。
        if placement.is_none() {
            // 重新读取可能因初始 span 而增长的行数。
            let current_rows = occupied.len() / column_count;
            // 有剩余行预算时才尝试第二次搜索。
            if current_rows < row_limit {
                // 一次扩展至多一个 span，并在行上限处停止。
                let expanded_rows = current_rows.saturating_add(row_span).min(row_limit);
                // 扩容乘积继续受总单元格预算保护。
                occupied.resize(column_count * expanded_rows, false);
                // 新增行可能与原矩阵底部共同形成更早的合法矩形，因此重新按行优先搜索。
                placement = find_available_rect(occupied, column_count, col_span, row_span, prefix);
            }
        }
        // 资源窗口内没有完整空闲矩形时，该子项保留零 frame。
        let Some((col, row)) = placement else {
            // 不通过重叠或无界扩行伪造自动放置成功。
            continue;
        };
        // 搜索结果与已归一 span 之和位于当前物化矩阵内。
        let needed_rows = row + row_span;
        // 登记自动子项的完整占用矩形。
        mark_occupied(occupied, column_count, col, row, col_span, row_span);
        // 只有成功放置的子项才会推高最终行数。
        used_rows = used_rows.max(needed_rows);
        // 保存自动放置的有界结果。
        assignments.push(CellAssignment {
            // 保留原始子项顺序索引。
            child_idx,
            // 写入搜索得到的列起点。
            col,
            // 写入搜索得到的行起点。
            row,
            // 有界列 span 可无损转为 u32。
            col_span: col_span as u32,
            // 有界行 span 可无损转为 u32。
            row_span: row_span as u32,
        });
    }

    // 定位账本保留在调用方工作区，只返回不超过轨道预算的行数。
    used_rows
}

// 放置算法的资源上限与极值输入回归。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/ui/layout/grid/placement/tests.rs"]
mod tests;
