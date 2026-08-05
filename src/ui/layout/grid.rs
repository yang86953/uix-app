use super::*;

/// Compute grid layout from input constraints.
/// Pure function: no side effects.
pub fn compute_grid_layout(input: &GridInput<'_>) -> GridOutput {
    let inner = Rect::new(
        input.container.x + input.padding.left,
        input.container.y + input.padding.top,
        (input.container.w - input.padding.horizontal()).max(0.0),
        (input.container.h - input.padding.vertical()).max(0.0),
    );

    let n_cols = input.columns.len();
    if n_cols == 0 || input.children.is_empty() {
        return GridOutput {
            child_rects: Vec::new(),
            total_size: Size::new(inner.w, inner.h),
        };
    }

    // ── Phase 1: register explicit placements ──
    let init_rows = input.rows.len().max(1);
    let mut occupied = vec![false; n_cols * init_rows];
    let mut assignments: Vec<CellAssignment> = Vec::with_capacity(input.children.len());

    // 先登记显式 cell，同时扩展 occupied 矩阵
    for (ci, child) in input.children.iter().enumerate() {
        if let Some(cell) = child.cell {
            let col = cell % n_cols;
            let row = cell / n_cols;
            let span_cols = (child.col_span as usize).clamp(1, n_cols - col);
            let span_rows = (child.row_span as usize).max(1);
            let needed_rows = row.saturating_add(span_rows);
            let cur_rows = occupied.len() / n_cols;
            if needed_rows > cur_rows {
                occupied.resize(n_cols * needed_rows, false);
            }
            for r in 0..span_rows {
                for c in 0..span_cols {
                    occupied[(row + r) * n_cols + (col + c)] = true;
                }
            }
            assignments.push(CellAssignment {
                child_idx: ci,
                col,
                row,
                col_span: span_cols as u32,
                row_span: span_rows as u32,
            });
        }
    }

    // 再为 auto 项搜索完整可用矩形
    for (ci, child) in input
        .children
        .iter()
        .enumerate()
        .filter(|(_, child)| child.cell.is_none())
    {
        // A span wider than the explicit grid can never fit and previously made
        // the row-growth search loop forever. CSS-like grids clamp it to the
        // available explicit columns while keeping row span semantics intact.
        let span_cols = (child.col_span as usize).clamp(1, n_cols);
        let span_rows = (child.row_span as usize).max(1);
        // 搜索下一个完整可用矩形
        let (col, row) = 'search: loop {
            let mut cur_rows = occupied.len() / n_cols;
            if span_rows > cur_rows {
                occupied.resize(n_cols * span_rows, false);
                cur_rows = span_rows;
            }
            for base_row in 0..cur_rows.max(1) {
                'row_search: for base_col in 0..n_cols {
                    // 验证完整 span 是否可用
                    if base_col + span_cols > n_cols {
                        continue 'row_search;
                    }
                    if base_row + span_rows > cur_rows.max(1) {
                        // 需要扩展行
                        break 'row_search;
                    }
                    for r in 0..span_rows {
                        for c in 0..span_cols {
                            if occupied[(base_row + r) * n_cols + (base_col + c)] {
                                continue 'row_search;
                            }
                        }
                    }
                    break 'search (base_col, base_row);
                }
            }
            // 当前矩阵没有可用矩形，扩展一行继续搜索。
            let cur_rows = occupied.len() / n_cols;
            occupied.resize(n_cols * (cur_rows + 1), false);
        };

        // 确保有足够行
        let needed_rows = row + span_rows;
        let cur_rows = occupied.len() / n_cols;
        if needed_rows > cur_rows {
            occupied.resize(n_cols * needed_rows, false);
        }

        for r in 0..span_rows {
            for c in 0..span_cols {
                occupied[(row + r) * n_cols + (col + c)] = true;
            }
        }

        assignments.push(CellAssignment {
            child_idx: ci,
            col,
            row,
            col_span: span_cols as u32,
            row_span: span_rows as u32,
        });
    }

    let n_rows = occupied.len() / n_cols;

    // ── Phase 2: build full rows (explicit + implicit auto) ──
    let mut rows = input.rows.to_vec();
    rows.resize(n_rows, GridTrack::Auto);

    // ── Phase 3: resolve track sizes ──
    let col_gap = finite_non_negative(input.col_gap);
    let row_gap = finite_non_negative(input.row_gap);
    let total_col_gap = col_gap * (n_cols.saturating_sub(1)) as f32;
    let total_row_gap = row_gap * (n_rows.saturating_sub(1)) as f32;

    // 先汇总子项在自动列上的固有宽度贡献。
    let col_auto_sizes =
        intrinsic_auto_track_sizes(input.columns, &assignments, input.children, col_gap, true);
    // 再汇总子项在显式与隐式自动行上的固有高度贡献。
    let row_auto_sizes =
        intrinsic_auto_track_sizes(&rows, &assignments, input.children, row_gap, false);
    // Auto 保留内容宽度，Fr 列仅分配剩余水平空间。
    let col_sizes = resolve_tracks(input.columns, inner.w, total_col_gap, &col_auto_sizes);
    // Auto 保留内容高度，Fr 行仅分配剩余垂直空间。
    let row_sizes = resolve_tracks(&rows, inner.h, total_row_gap, &row_auto_sizes);

    // ── Phase 4: build cell positions ──
    let mut col_positions: Vec<(f32, f32)> = Vec::with_capacity(n_cols);

    let mut cx = inner.x;
    for (index, size) in col_sizes.iter().copied().enumerate() {
        col_positions.push((cx, size));
        cx += size;
        if index + 1 < n_cols {
            cx += col_gap;
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

    let total_w = col_positions
        .last()
        .map(|(x, w)| x + w - inner.x)
        .unwrap_or(0.0)
        + input.padding.horizontal();
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

fn finite_non_negative(value: f32) -> f32 {
    // 非有限值与 f32::MAX 测量哨兵都不能进入实际布局算术。
    if value.is_finite() && value.abs() < f32::MAX {
        value.max(0.0)
    } else {
        0.0
    }
}

fn finite_or_zero(value: f32) -> f32 {
    // 有限负 margin 保持既有语义，无界哨兵与非有限值回退为零。
    if value.is_finite() && value.abs() < f32::MAX {
        value
    } else {
        0.0
    }
}

fn finite_insets(insets: EdgeInsets) -> EdgeInsets {
    EdgeInsets::new(
        finite_or_zero(insets.left),
        finite_or_zero(insets.top),
        finite_or_zero(insets.right),
        finite_or_zero(insets.bottom),
    )
}

// 计算单个子项在指定轴上的有限外尺寸。
fn child_outer_extent(child: &GridChild, horizontal: bool) -> f32 {
    // 水平轴取测量宽度，垂直轴取测量高度。
    let measured = if horizontal {
        // 拒绝宽度中的无界哨兵和非有限值。
        finite_non_negative(child.measured_size.w)
    } else {
        // 拒绝高度中的无界哨兵和非有限值。
        finite_non_negative(child.measured_size.h)
    };
    // 先将外边距收敛为可参与布局的有限值。
    let margin = finite_insets(child.margin);
    // 水平轴取左右外边距，垂直轴取上下外边距。
    let margin_extent = if horizontal {
        // 宽度贡献包含左右外边距。
        margin.horizontal()
    } else {
        // 高度贡献包含上下外边距。
        margin.vertical()
    };
    // 使用有限化边界防止外尺寸溢出或变为负数。
    finite_non_negative(measured + margin_extent)
}

// 汇总 Auto 轨道的单格内容与跨格内容贡献。
fn intrinsic_auto_track_sizes(
    tracks: &[GridTrack],
    assignments: &[CellAssignment],
    children: &[GridChild],
    gap: f32,
    horizontal: bool,
) -> Vec<f32> {
    // 为每条轨道建立独立的固有尺寸账本。
    let mut sizes = vec![0.0f32; tracks.len()];

    // 先用单轨道子项确定每条 Auto 轨道的基础尺寸。
    for assignment in assignments {
        // 按当前轴选取起始轨道。
        let start = if horizontal {
            // 水平轴使用列起点。
            assignment.col
        } else {
            // 垂直轴使用行起点。
            assignment.row
        };
        // 按当前轴选取跨越的轨道数。
        let span = if horizontal {
            // 水平轴使用列 span。
            assignment.col_span as usize
        } else {
            // 垂直轴使用行 span。
            assignment.row_span as usize
        };
        // 将结束位置限制在实际轨道数量内。
        let end = start.saturating_add(span).min(tracks.len());
        // 这一轮只处理恰好占用一条轨道的子项。
        if end.saturating_sub(start) != 1 {
            // 多轨道子项留到第二轮分配。
            continue;
        }
        // 固定和比例轨道不由固有内容改写。
        if !matches!(tracks[start], GridTrack::Auto) {
            // 非 Auto 轨道继续使用自身定义。
            continue;
        }
        // 读取该子项在当前轴上的有限外尺寸。
        let extent = child_outer_extent(&children[assignment.child_idx], horizontal);
        // 同一 Auto 轨道取所有单格子项的最大贡献。
        sizes[start] = sizes[start].max(extent);
    }

    // 再将跨格子项的尺寸缺口分摊到它覆盖的 Auto 轨道。
    for assignment in assignments {
        // 按当前轴选取起始轨道。
        let start = if horizontal {
            // 水平轴使用列起点。
            assignment.col
        } else {
            // 垂直轴使用行起点。
            assignment.row
        };
        // 按当前轴选取跨越的轨道数。
        let span = if horizontal {
            // 水平轴使用列 span。
            assignment.col_span as usize
        } else {
            // 垂直轴使用行 span。
            assignment.row_span as usize
        };
        // 将结束位置限制在实际轨道数量内。
        let end = start.saturating_add(span).min(tracks.len());
        // 单轨道子项已经在第一轮处理。
        if end.saturating_sub(start) <= 1 {
            // 不对空 span 或单轨道 span 重复计费。
            continue;
        }
        // 含正比例 Fr 的 span 由后续剩余空间分配承担。
        let contains_fraction = tracks[start..end].iter().any(|track| {
            // 只有正且有限的 Fr 权重会参与空间分配。
            matches!(track, GridTrack::Fr(fr) if finite_non_negative(*fr) > 0.0)
        });
        // 保留 Fr 轨道吸收剩余空间的既有语义。
        if contains_fraction {
            // 这类 span 不额外扩张 Auto 轨道。
            continue;
        }
        // 统计 span 中可承担缺口的 Auto 轨道数。
        let auto_count = tracks[start..end]
            .iter()
            .filter(|track| matches!(track, GridTrack::Auto))
            .count();
        // 没有 Auto 轨道时不应改写固定轨道。
        if auto_count == 0 {
            // 当前 span 没有可分摊的轨道。
            continue;
        }
        // 用 f64 账本避免多条轨道求和时提前溢出。
        let track_extent = tracks[start..end]
            .iter()
            .enumerate()
            .map(|(offset, track)| {
                // 固定轨道计入自身宽高，Auto 计入已汇总的固有尺寸。
                match track {
                    // 固定轨道使用安全化后的像素值。
                    GridTrack::Px(px) => finite_non_negative(*px) as f64,
                    // 当前 Auto 轨道使用第一轮已确定的尺寸。
                    GridTrack::Auto => sizes[start + offset] as f64,
                    // 非正 Fr 在这一固有尺寸账本中不占空间。
                    GridTrack::Fr(_) => 0.0,
                }
            })
            .sum::<f64>();
        // span 内部的 gap 同样属于子项可用外尺寸。
        let gap_extent = gap as f64 * end.saturating_sub(start + 1) as f64;
        // 读取跨格子项在当前轴上的有限外尺寸。
        let required = child_outer_extent(&children[assignment.child_idx], horizontal) as f64;
        // 只需补足现有轨道和 gap 尚未覆盖的尺寸。
        let deficit = (required - track_extent - gap_extent).max(0.0);
        // 没有缺口时保留现有轨道尺寸。
        if deficit <= 0.0 {
            // 当前 span 已能容纳子项。
            continue;
        }
        // 将缺口均匀分摊给 span 中的 Auto 轨道。
        let share = deficit / auto_count as f64;
        // 逐条更新 span 中的 Auto 轨道。
        for index in start..end {
            // 固定和比例轨道不承担 Auto 尺寸缺口。
            if matches!(tracks[index], GridTrack::Auto) {
                // 将累加结果收敛到有限非负尺寸。
                sizes[index] = finite_non_negative((sizes[index] as f64 + share) as f32);
            }
        }
    }

    // 返回仅对 Auto 轨道有意义的固有尺寸账本。
    sizes
}

// 在固定与 Auto 尺寸确定后把剩余空间分配给 Fr 轨道。
fn resolve_tracks(
    tracks: &[GridTrack],
    available: f32,
    total_gap: f32,
    auto_sizes: &[f32],
) -> Vec<f32> {
    // 为每条轨道建立最终尺寸账本。
    let mut sizes = vec![0.0; tracks.len()];
    // 使用 f64 累加已确定尺寸，避免多轨道求和溢出。
    let mut used = 0.0f64;
    // 使用 f64 累加比例权重，避免极大权重求和溢出。
    let mut total_fr = 0.0f64;

    // 先锁定 Px 和 Auto 轨道，并汇总有效 Fr 权重。
    for (index, track) in tracks.iter().enumerate() {
        // 按轨道类型建立基础尺寸。
        match track {
            // 固定轨道直接使用有限非负像素值。
            GridTrack::Px(px) => {
                // 收敛外部传入的固定尺寸。
                let size = finite_non_negative(*px);
                // 写入当前轨道的最终尺寸。
                sizes[index] = size;
                // 固定尺寸优先占用可用空间。
                used += size as f64;
            }
            // 自动轨道使用上一阶段的内容尺寸。
            GridTrack::Auto => {
                // 缺失的账本项安全回退为零。
                let size = auto_sizes
                    .get(index)
                    .copied()
                    .map(finite_non_negative)
                    .unwrap_or(0.0);
                // 写入当前 Auto 轨道的最终尺寸。
                sizes[index] = size;
                // Auto 内容尺寸先于 Fr 占用可用空间。
                used += size as f64;
            }
            // 比例轨道留到第二轮分配剩余空间。
            GridTrack::Fr(fr) => {
                // 仅累加正且有限的比例权重。
                total_fr += finite_non_negative(*fr) as f64;
            }
        }
    }

    // 可用空间先扣除 gap、固定轨道和 Auto 内容尺寸。
    let remaining =
        (finite_non_negative(available) as f64 - finite_non_negative(total_gap) as f64 - used)
            .max(0.0);
    // 没有可分配空间或有效 Fr 权重时直接保留基础尺寸。
    if remaining <= 0.0 || total_fr <= 0.0 {
        // 纯 Auto 网格因此不会无条件填满父容器。
        return sizes;
    }

    // 按权重将全部剩余空间分配给 Fr 轨道。
    for (index, track) in tracks.iter().enumerate() {
        // 只有 Fr 轨道需要在这一轮更新。
        if let GridTrack::Fr(fr) = track {
            // 将当前轨道权重收敛为有限非负值。
            let weight = finite_non_negative(*fr) as f64;
            // 按权重比例写入该 Fr 轨道的最终尺寸。
            sizes[index] = finite_non_negative((remaining * weight / total_fr) as f32);
        }
    }

    // 返回已完成 Px、Auto 与 Fr 分配的轨道尺寸。
    sizes
}

struct CellAssignment {
    child_idx: usize,
    col: usize,
    row: usize,
    col_span: u32,
    row_span: u32,
}
