use super::*;

/// Compute grid layout from input constraints.
/// Pure function: no side effects.
pub fn compute_grid_layout(input: &GridInput) -> GridOutput {
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
    let mut occupied: Vec<bool> = Vec::new();
    let mut assignments: Vec<CellAssignment> = Vec::with_capacity(input.children.len());
    let mut auto_items: Vec<(usize, &GridChild)> = Vec::new();

    // 先登记显式 cell，同时扩展 occupied 矩阵
    for (ci, child) in input.children.iter().enumerate() {
        if let Some(cell) = child.cell {
            let col = cell % n_cols;
            let row = cell / n_cols;
            let needed_rows = row + child.row_span as usize;
            let cur_rows = occupied.len() / n_cols;
            if needed_rows > cur_rows {
                occupied.resize(n_cols * needed_rows, false);
            }
            let span_cols = (child.col_span as usize).min(n_cols - col);
            let span_rows = (child.row_span as usize).min(needed_rows - row);
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
        } else {
            auto_items.push((ci, child));
        }
    }

    // 再为 auto 项搜索完整可用矩形
    for (ci, child) in auto_items {
        let span_cols = child.col_span as usize;
        let span_rows = child.row_span as usize;
        // 搜索下一个完整可用矩形
        let (col, row) = 'search: loop {
            let cur_rows = occupied.len() / n_cols;
            let mut found = false;
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
                    let mut ok = true;
                    for r in 0..span_rows {
                        for c in 0..span_cols {
                            if occupied[(base_row + r) * n_cols + (base_col + c)] {
                                ok = false;
                                break 'row_search;
                            }
                        }
                    }
                    if ok {
                        found = true;
                        break 'search (base_col, base_row);
                    }
                }
            }
            if !found {
                // 扩展一行继续搜索
                let cur_rows = occupied.len() / n_cols;
                occupied.resize(n_cols * (cur_rows + 1), false);
            }
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
    let mut rows = input.rows.clone();
    rows.resize(n_rows, GridTrack::Auto);

    // ── Phase 3: resolve track sizes ──
    let total_col_gap = input.col_gap * (n_cols.saturating_sub(1)) as f32;
    let total_row_gap = input.row_gap * (n_rows.saturating_sub(1)) as f32;

    let resolve_tracks = |tracks: &[GridTrack], available: f32, total_gap: f32| -> Vec<f32> {
        let mut sizes = vec![0.0f32; tracks.len()];
        let mut used = 0.0f32;
        let mut total_fr = 0.0f32;
        let mut auto_count = 0usize;

        for track in tracks.iter() {
            match track {
                GridTrack::Px(px) => {
                    used += px;
                }
                GridTrack::Fr(fr) => {
                    total_fr += fr;
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
                    GridTrack::Px(px) => sizes[i] = *px,
                    GridTrack::Fr(fr) => sizes[i] = unit * fr,
                    GridTrack::Auto => sizes[i] = unit,
                }
            }
        } else {
            for (i, track) in tracks.iter().enumerate() {
                if let GridTrack::Px(px) = track {
                    sizes[i] = *px;
                }
            }
        }
        sizes
    };

    let col_sizes = resolve_tracks(&input.columns, inner.w, total_col_gap);
    let row_sizes = resolve_tracks(&rows, inner.h, total_row_gap);

    // ── Phase 4: build cell positions ──
    let mut col_positions: Vec<(f32, f32)> = Vec::with_capacity(n_cols);

    let mut cx = inner.x;
    for i in 0..n_cols {
        col_positions.push((cx, col_sizes[i]));
        cx += col_sizes[i];
        if i < n_cols - 1 {
            cx += input.col_gap;
        }
    }

    let mut row_positions: Vec<(f32, f32)> = Vec::with_capacity(n_rows);
    let mut cy = inner.y;
    for i in 0..n_rows {
        row_positions.push((cy, row_sizes[i]));
        cy += row_sizes[i];
        if i < n_rows - 1 {
            cy += input.row_gap;
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

        let mut cell_w = 0.0f32;
        for c in 0..assignment.col_span as usize {
            if assignment.col + c < col_positions.len() {
                cell_w += col_positions[assignment.col + c].1;
                if c > 0 {
                    cell_w += input.col_gap;
                }
            }
        }
        let mut cell_h = 0.0f32;
        for r in 0..assignment.row_span as usize {
            if assignment.row + r < row_positions.len() {
                cell_h += row_positions[assignment.row + r].1;
                if r > 0 {
                    cell_h += input.row_gap;
                }
            }
        }

        let h_align = child.justify.unwrap_or(input.justify_items);
        let v_align = child.align.unwrap_or(input.align_items);
        let pref = child.measured_size;

        let child_w = match h_align {
            JustifyContent::Start => pref.w.min(cell_w),
            _ => cell_w,
        };
        let child_h = match v_align {
            AlignItems::Start => pref.h.min(cell_h),
            _ => cell_h,
        };
        let child_x = match h_align {
            JustifyContent::Start => cell_x,
            JustifyContent::Center => cell_x + (cell_w - child_w) * 0.5,
            JustifyContent::End => cell_x + cell_w - child_w,
            _ => cell_x,
        };
        let child_y = match v_align {
            AlignItems::Start => cell_y,
            AlignItems::Center => cell_y + (cell_h - child_h) * 0.5,
            AlignItems::End => cell_y + cell_h - child_h,
            _ => cell_y,
        };

        child_rects[assignment.child_idx] = Rect::new(child_x, child_y, child_w, child_h);
    }

    GridOutput {
        child_rects,
        total_size: Size::new(total_w, total_h),
    }
}

struct CellAssignment {
    child_idx: usize,
    col: usize,
    row: usize,
    col_span: u32,
    row_span: u32,
}
