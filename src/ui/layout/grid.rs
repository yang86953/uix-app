use super::*;

// 将 Grid 放置的轨道与单元格资源契约集中在独立模块。
mod placement;
// 将 Auto/Fr 轨道尺寸求解集中在独立模块。
mod track_sizing;
// 将整组列轨的水平内容对齐集中在独立模块。
mod justify;
// 引入有界列数、放置计划与后续尺寸求解所需的定位类型。
use placement::{bounded_column_count, place_grid_children, CellAssignment};
// 引入拆分后的轨道尺寸求解与有限化辅助。
use track_sizing::{
    finite_insets, finite_non_negative, fit_fraction_spanning_auto_tracks,
    intrinsic_auto_track_sizes, resolve_tracks,
};
// 引入拆分后的列轨内容对齐入口。
use justify::justify_grid_content;

/// Pure function: no side effects.
pub(crate) fn compute_grid_layout(input: &GridInput<'_>) -> GridOutput {
    let inner = Rect::new(
        input.container.x + input.padding.left,
        input.container.y + input.padding.top,
        (input.container.w - input.padding.horizontal()).max(0.0),
        (input.container.h - input.padding.vertical()).max(0.0),
    );

    // 列轨道同样纳入单轴资源上限。
    let n_cols = bounded_column_count(input.columns.len());
    if n_cols == 0 || input.children.is_empty() {
        return GridOutput {
            child_rects: Vec::new(),
            total_size: Size::new(inner.w, inner.h),
        };
    }

    // 后续所有列尺寸与索引都只能使用收敛后的列窗口。
    let columns = &input.columns[..n_cols];
    // ── Phase 1: resolve bounded explicit and automatic placements ──
    // 统一收敛 cell、span、行数、占用矩阵与自动搜索。
    let placement = place_grid_children(n_cols, input.rows.len(), input.children);
    // 后续轨道尺寸求解沿用有界定位账本。
    let assignments = placement.assignments;
    // 只物化显式行与成功放置子项真正需要的行。
    let n_rows = placement.row_count;

    // ── Phase 2: build full rows (explicit + implicit auto) ──
    // 显式行只复制有界求解窗口内的部分。
    let mut rows: Vec<GridTrack> = input.rows.iter().copied().take(n_rows).collect();
    // 成功放置产生的隐式行继续使用 Auto 轨道。
    rows.resize(n_rows, GridTrack::Auto);

    // ── Phase 3: resolve track sizes ──
    let col_gap = finite_non_negative(input.col_gap);
    let row_gap = finite_non_negative(input.row_gap);
    let total_col_gap = col_gap * (n_cols.saturating_sub(1)) as f32;
    let total_row_gap = row_gap * (n_rows.saturating_sub(1)) as f32;

    // 先汇总子项在自动列上的固有宽度贡献。
    let mut col_auto_sizes =
        intrinsic_auto_track_sizes(columns, &assignments, input.children, col_gap, true);
    // 把 span 外 Fr 可让出的份额转给 span 内 Auto，避免混合跨轨子项被竞争 Fr 裁剪。
    fit_fraction_spanning_auto_tracks(
        // 水平轴使用有界列定义。
        columns,
        // 复用放置阶段的有界子项账本。
        &assignments,
        // 读取子项自然外宽。
        input.children,
        // 列间距属于跨轨可用宽度。
        col_gap,
        // 水平容器约束限制可转移空间。
        inner.w,
        // 全部列间距先于 Fr 分配扣除。
        total_col_gap,
        // 选择水平尺寸分支。
        true,
        // 原位扩展 Auto 列固有尺寸。
        &mut col_auto_sizes,
    );
    // 再汇总子项在显式与隐式自动行上的固有高度贡献。
    let mut row_auto_sizes =
        intrinsic_auto_track_sizes(&rows, &assignments, input.children, row_gap, false);
    // 行轴使用相同规则处理 Auto/Fr 混合跨轨约束。
    fit_fraction_spanning_auto_tracks(
        // 垂直轴使用显式与隐式行定义。
        &rows,
        // 复用同一放置账本。
        &assignments,
        // 读取子项自然外高。
        input.children,
        // 行间距属于跨轨可用高度。
        row_gap,
        // 垂直容器约束限制可转移空间。
        inner.h,
        // 全部行间距先于 Fr 分配扣除。
        total_row_gap,
        // 选择垂直尺寸分支。
        false,
        // 原位扩展 Auto 行固有尺寸。
        &mut row_auto_sizes,
    );
    // Auto 保留内容宽度，Fr 列仅分配剩余水平空间。
    let mut col_sizes = resolve_tracks(columns, inner.w, total_col_gap, &col_auto_sizes);
    // 在轨道尺寸确定后应用整组列轨的水平内容对齐。
    let (content_offset, distributed_col_gap, content_uses_available_width) = justify_grid_content(
        // 原始轨道类型用于识别 Stretch 可扩展的 Auto 列。
        columns,
        // 内容对齐可以原位扩展 Auto 列。
        &mut col_sizes,
        // 父级内容宽度提供可分布空间。
        inner.w,
        // 作者声明的基础 gap 先从可用空间扣除。
        total_col_gap,
        // 使用独立的 Grid 内容对齐声明。
        input.justify_content,
    );
    // Auto 保留内容高度，Fr 行仅分配剩余垂直空间。
    let row_sizes = resolve_tracks(&rows, inner.h, total_row_gap, &row_auto_sizes);

    // ── Phase 4: build cell positions ──
    let mut col_positions: Vec<(f32, f32)> = Vec::with_capacity(n_cols);

    let mut cx = inner.x + content_offset;
    for (index, size) in col_sizes.iter().copied().enumerate() {
        col_positions.push((cx, size));
        cx += size;
        if index + 1 < n_cols {
            // 分布型内容对齐只增加轨道间距，不改写作者基础 gap。
            cx += col_gap + distributed_col_gap;
        }
    }

    let mut row_positions: Vec<(f32, f32)> = Vec::with_capacity(n_rows);
    let mut cy = inner.y;
    for (index, size) in row_sizes.iter().copied().enumerate() {
        row_positions.push((cy, size));
        cy += size;
        if index + 1 < n_rows {
            cy += row_gap;
        }
    }

    // 先按最终列轨位置计算从内容起点到末轨的物理占用。
    let positioned_total_w = col_positions
        .last()
        .map(|(x, w)| x + w - inner.x)
        .unwrap_or(0.0);
    // 使用剩余空间的对齐模式必须把前后分布空间纳入总宽度。
    let total_w = if content_uses_available_width {
        // Center/End/Space*/有效 Stretch 都以完整父级宽度为占用边界。
        inner.w + input.padding.horizontal()
    } else {
        // Start 与无 Auto 的 Stretch 保留既有自然轨道宽度。
        positioned_total_w + input.padding.horizontal()
    };
    let total_h = row_positions
        .last()
        .map(|(y, h)| y + h - inner.y)
        .unwrap_or(0.0)
        + input.padding.vertical();

    // ── Phase 5: compute child rects ──
    let mut child_rects = vec![Rect::zero(); input.children.len()];

    for assignment in &assignments {
        let child = &input.children[assignment.child_idx];
        let cell_x = col_positions[assignment.col].0;
        let cell_y = row_positions[assignment.row].0;

        let col_end = (assignment.col + assignment.col_span as usize).min(col_positions.len());
        let row_end = (assignment.row + assignment.row_span as usize).min(row_positions.len());
        let cell_w = col_positions[col_end - 1].0 + col_positions[col_end - 1].1 - cell_x;
        let cell_h = row_positions[row_end - 1].0 + row_positions[row_end - 1].1 - cell_y;

        let h_align = child.justify.unwrap_or(input.justify_items);
        let v_align = child.align.unwrap_or(input.align_items);
        let pref = Size::new(
            finite_non_negative(child.measured_size.w),
            finite_non_negative(child.measured_size.h),
        );
        let margin = finite_insets(child.margin);
        let available_w = (cell_w - margin.horizontal()).max(0.0);
        let available_h = (cell_h - margin.vertical()).max(0.0);

        let child_w = match h_align {
            JustifyContent::Start | JustifyContent::Center | JustifyContent::End => {
                pref.w.min(available_w)
            }
            _ => available_w,
        };
        let child_h = match v_align {
            AlignItems::Start | AlignItems::Center | AlignItems::End => pref.h.min(available_h),
            AlignItems::Stretch => available_h,
        };
        let child_x = match h_align {
            JustifyContent::Start => cell_x + margin.left,
            JustifyContent::Center => cell_x + margin.left + (available_w - child_w) * 0.5,
            JustifyContent::End => cell_x + margin.left + available_w - child_w,
            _ => cell_x + margin.left,
        };
        let child_y = match v_align {
            AlignItems::Start => cell_y + margin.top,
            AlignItems::Center => cell_y + margin.top + (available_h - child_h) * 0.5,
            AlignItems::End => cell_y + margin.top + available_h - child_h,
            AlignItems::Stretch => cell_y + margin.top,
        };

        child_rects[assignment.child_idx] = Rect::new(child_x, child_y, child_w, child_h);
    }

    GridOutput {
        child_rects,
        total_size: Size::new(total_w, total_h),
    }
}

// 计算整组 Grid 列轨的水平内容偏移、附加间距与占用边界。
