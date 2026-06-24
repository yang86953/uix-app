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
            child_rects: Vec::new(), col_positions: Vec::new(),
            row_positions: Vec::new(), total_size: Size::new(inner.w, inner.h),
        };
    }

    // ── Phase 1: auto-place children, create implicit rows as needed ──
    let init_rows = input.rows.len().max(1);
    let mut occupied = vec![false; n_cols * init_rows];
    let mut assignments = Vec::with_capacity(input.children.len());
    let mut next_cell = 0;

    for (ci, child) in input.children.iter().enumerate() {
        let (start_cell, col, row) = if child.cell > 0 {
            let cell = child.cell;
            let min_rows = (cell / n_cols) + (child.row_span as usize);
            let cur_rows = occupied.len() / n_cols;
            if min_rows > cur_rows {
                occupied.resize(n_cols * min_rows, false);
            }
            (cell, cell % n_cols, cell / n_cols)
        } else {
            while next_cell < occupied.len() && occupied[next_cell] {
                next_cell += 1;
            }
            // 确保有足够行容纳 row_span（与手动 cell 指定路径的 min_rows 逻辑一致）
            let needed_rows = (next_cell / n_cols) + child.row_span as usize;
            let cur_rows = occupied.len() / n_cols;
            if needed_rows > cur_rows {
                occupied.resize(n_cols * needed_rows, false);
            }
            let cell = next_cell;
            (cell, cell % n_cols, cell / n_cols)
        };

        let total_rows = occupied.len() / n_cols;
        let span_cols = (child.col_span as usize).min(n_cols - col);
        let span_rows = (child.row_span as usize).min(total_rows - row);

        for r in 0..span_rows {
            for c in 0..span_cols {
                occupied[(row + r) * n_cols + (col + c)] = true;
            }
        }

        assignments.push(CellAssignment {
            child_idx: ci, col, row,
            col_span: span_cols as u32,
            row_span: span_rows as u32,
        });
        next_cell = start_cell + span_cols;
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
                GridTrack::Px(px) => { used += px; }
                GridTrack::Fr(fr) => { total_fr += fr; }
                GridTrack::Auto => { auto_count += 1; }
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
                if let GridTrack::Px(px) = track { sizes[i] = *px; }
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
        if i < n_cols - 1 { cx += input.col_gap; }
    }

    let mut row_positions: Vec<(f32, f32)> = Vec::with_capacity(n_rows);
    let mut cy = inner.y;
    for i in 0..n_rows {
        row_positions.push((cy, row_sizes[i]));
        cy += row_sizes[i];
        if i < n_rows - 1 { cy += input.row_gap; }
    }

    let total_w = col_positions.last().map(|(x,w)| x + w - inner.x).unwrap_or(0.0) + input.padding.horizontal();
    let total_h = row_positions.last().map(|(y,h)| y + h - inner.y).unwrap_or(0.0) + input.padding.vertical();

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
                if c > 0 { cell_w += input.col_gap; }
            }
        }
        let mut cell_h = 0.0f32;
        for r in 0..assignment.row_span as usize {
            if assignment.row + r < row_positions.len() {
                cell_h += row_positions[assignment.row + r].1;
                if r > 0 { cell_h += input.row_gap; }
            }
        }

        let h_align = child.justify.unwrap_or(input.justify_items);
        let v_align = child.align.unwrap_or(input.align_items);
        let pref = child.preferred_size;

        let child_w = match h_align { JustifyContent::Start => pref.w.min(cell_w), _ => cell_w };
        let child_h = match v_align { AlignItems::Start => pref.h.min(cell_h), _ => cell_h };
        let child_x = match h_align { JustifyContent::Start => cell_x, JustifyContent::Center => cell_x+(cell_w-child_w)*0.5, JustifyContent::End => cell_x+cell_w-child_w, _ => cell_x };
        let child_y = match v_align { AlignItems::Start => cell_y, AlignItems::Center => cell_y+(cell_h-child_h)*0.5, AlignItems::End => cell_y+cell_h-child_h, _ => cell_y };

        child_rects[assignment.child_idx] = Rect::new(child_x, child_y, child_w, child_h);
    }

    GridOutput { child_rects, col_positions, row_positions, total_size: Size::new(total_w, total_h) }
}

struct CellAssignment {
    child_idx: usize, col: usize, row: usize, col_span: u32, row_span: u32,
}

#[cfg(test)]
mod grid_tests {
    use super::*;

    #[test] fn grid_2x2_fixed() {
        let o = compute_grid_layout(&GridInput {
            container: Rect::new(0.0,0.0,200.0,200.0),
            columns: vec![GridTrack::Px(100.0),GridTrack::Px(100.0)],
            rows: vec![GridTrack::Px(100.0),GridTrack::Px(100.0)],
            align_items: AlignItems::Stretch, justify_items: JustifyContent::Stretch,
            children: vec![GridChild{preferred_size:Size::new(50.0,50.0),..Default::default()};4],
            ..Default::default()
        });
        assert_eq!(o.child_rects[0], Rect::new(0.0,0.0,100.0,100.0));
        assert_eq!(o.child_rects[1], Rect::new(100.0,0.0,100.0,100.0));
        assert_eq!(o.child_rects[2], Rect::new(0.0,100.0,100.0,100.0));
        assert_eq!(o.child_rects[3], Rect::new(100.0,100.0,100.0,100.0));
    }
    #[test] fn grid_fraction_columns() {
        let o = compute_grid_layout(&GridInput {
            container: Rect::new(0.0,0.0,300.0,100.0),
            columns: vec![GridTrack::Fr(1.0),GridTrack::Fr(2.0)],
            rows: vec![GridTrack::Px(100.0)],
            align_items: AlignItems::Stretch, justify_items: JustifyContent::Stretch,
            children: vec![GridChild{preferred_size:Size::new(50.0,50.0),..Default::default()};2],
            ..Default::default()
        });
        assert!((o.child_rects[0].w - 100.0).abs() < 1.0);
        assert!((o.child_rects[1].w - 200.0).abs() < 1.0);
    }
    #[test] fn grid_with_gap() {
        let o = compute_grid_layout(&GridInput {
            container: Rect::new(0.0,0.0,210.0,210.0),
            columns: vec![GridTrack::Px(100.0),GridTrack::Px(100.0)],
            rows: vec![GridTrack::Px(100.0),GridTrack::Px(100.0)],
            col_gap: 10.0, row_gap: 10.0,
            align_items: AlignItems::Stretch, justify_items: JustifyContent::Stretch,
            children: vec![GridChild{preferred_size:Size::new(50.0,50.0),..Default::default()}],
            ..Default::default()
        });
        assert_eq!(o.child_rects[0], Rect::new(0.0,0.0,100.0,100.0));
        assert!((o.col_positions[1].0 - 110.0).abs() < 1.0);
    }
    #[test] fn grid_empty() {
        let o = compute_grid_layout(&GridInput::default());
        assert!(o.child_rects.is_empty());
    }
    #[test] fn grid_fr_auto_mixed() {
        let o = compute_grid_layout(&GridInput {
            container: Rect::new(0.0,0.0,300.0,100.0),
            columns: vec![GridTrack::Fr(1.0), GridTrack::Auto],
            rows: vec![GridTrack::Px(100.0)],
            align_items: AlignItems::Stretch, justify_items: JustifyContent::Stretch,
            children: vec![
                GridChild { preferred_size: Size::new(50.0, 50.0), ..Default::default() },
                GridChild { preferred_size: Size::new(50.0, 50.0), ..Default::default() },
            ],
            ..Default::default()
        });
        // Fr(1) + Auto = 2 flexible units, 300/2 = 150 each
        assert!((o.child_rects[0].w - 150.0).abs() < 1.0);
        assert!((o.child_rects[1].w - 150.0).abs() < 1.0);
    }
    #[test] fn grid_implicit_rows() {
        let o = compute_grid_layout(&GridInput {
            container: Rect::new(0.0,0.0,200.0,200.0),
            columns: vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
            rows: vec![GridTrack::Px(100.0)], // 1 explicit row
            col_gap: 0.0, row_gap: 0.0,
            align_items: AlignItems::Stretch, justify_items: JustifyContent::Stretch,
            children: vec![
                GridChild { cell: 0, ..Default::default() }, // row 0, col 0
                GridChild { cell: 1, ..Default::default() }, // row 0, col 1
                GridChild { cell: 2, ..Default::default() }, // row 1, col 0 — implicit row
                GridChild { cell: 3, ..Default::default() }, // row 1, col 1 — implicit row
            ],
            ..Default::default()
        });
        assert_eq!(o.child_rects.len(), 4);
        assert_eq!(o.row_positions.len(), 2); // 1 explicit + 1 implicit
        assert_eq!(o.child_rects[2], Rect::new(0.0, 100.0, 100.0, 100.0));
        assert_eq!(o.child_rects[3], Rect::new(100.0, 100.0, 100.0, 100.0));
    }
}
