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

    let resolve_tracks = |tracks: &[GridTrack], available: f32, total_gap: f32| -> Vec<f32> {
        let mut sizes = vec![0.0f32; tracks.len()];
        let mut used = 0.0f32;
        let mut total_fr = 0.0f32;
        let mut auto_count = 0usize;

        for track in tracks.iter() {
            match track {
                GridTrack::Px(px) => {
                    used += finite_non_negative(*px);
                }
                GridTrack::Fr(fr) => {
                    total_fr += finite_non_negative(*fr);
                }
                GridTrack::Auto => {
                    auto_count += 1;
                }
            }
        }

        let remaining = (available - total_gap - used).max(0.0);
        let total_flexible = total_fr + auto_count as f32;
        if total_flexible > 0.0 && remaining > 0.0 {
            let unit = remaining / total_flexible;
            for (i, track) in tracks.iter().enumerate() {
                match track {
                    GridTrack::Px(px) => sizes[i] = finite_non_negative(*px),
                    GridTrack::Fr(fr) => sizes[i] = unit * finite_non_negative(*fr),
                    GridTrack::Auto => sizes[i] = unit,
                }
            }
        } else {
            for (i, track) in tracks.iter().enumerate() {
                if let GridTrack::Px(px) = track {
                    sizes[i] = finite_non_negative(*px);
                }
            }
        }
        sizes
    };

    let col_sizes = resolve_tracks(input.columns, inner.w, total_col_gap);
    let row_sizes = resolve_tracks(&rows, inner.h, total_row_gap);

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

struct CellAssignment {
    child_idx: usize,
    col: usize,
    row: usize,
    col_span: u32,
    row_span: u32,
}
