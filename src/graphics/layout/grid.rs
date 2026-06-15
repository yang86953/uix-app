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
    let n_rows = input.rows.len();

    if n_cols == 0 || n_rows == 0 || input.children.is_empty() {
        return GridOutput {
            child_rects: Vec::new(), col_positions: Vec::new(),
            row_positions: Vec::new(), total_size: Size::new(inner.w, inner.h),
        };
    }

    // Resolve track sizes
    let total_col_gap = input.col_gap * (n_cols.saturating_sub(1)) as f32;
    let total_row_gap = input.row_gap * (n_rows.saturating_sub(1)) as f32;

    let resolve_tracks = |tracks: &[GridTrack], available: f32, total_gap: f32| -> Vec<f32> {
        let mut sizes = vec![0.0f32; tracks.len()];
        let mut used = 0.0f32;
        let mut total_fr = 0.0f32;

        for (i, track) in tracks.iter().enumerate() {
            match track {
                GridTrack::Px(px) => { sizes[i] = *px; used += px; }
                GridTrack::Fr(fr) => { total_fr += fr; }
                GridTrack::Auto => { sizes[i] = 0.0; }
            }
        }

        let remaining = (available - total_gap - used).max(0.0);
        if total_fr > 0.0 && remaining > 0.0 {
            for (i, track) in tracks.iter().enumerate() {
                if let GridTrack::Fr(fr) = track { sizes[i] = remaining * fr / total_fr; }
            }
        }
        sizes
    };

    let col_sizes = resolve_tracks(&input.columns, inner.w, total_col_gap);
    let row_sizes = resolve_tracks(&input.rows, inner.h, total_row_gap);

    // Build cell positions
    let mut col_positions: Vec<(f32, f32)> = Vec::with_capacity(n_cols);
    let mut cx = inner.x;
    for (i, &cw) in col_sizes.iter().enumerate().take(n_cols) {
        col_positions.push((cx, cw));
        cx += cw;
        if i < n_cols - 1 { cx += input.col_gap; }
    }

    let mut row_positions: Vec<(f32, f32)> = Vec::with_capacity(n_rows);
    let mut cy = inner.y;
    for (i, &rh) in row_sizes.iter().enumerate().take(n_rows) {
        row_positions.push((cy, rh));
        cy += rh;
        if i < n_rows - 1 { cy += input.row_gap; }
    }

    let total_w = col_positions.last().map(|(x,w)| x + w - inner.x).unwrap_or(0.0) + input.padding.horizontal();
    let total_h = row_positions.last().map(|(y,h)| y + h - inner.y).unwrap_or(0.0) + input.padding.vertical();

    // Place children
    struct CellAssignment { child_idx: usize, col: usize, row: usize, col_span: u32, row_span: u32 }
    let mut assignments = Vec::with_capacity(input.children.len());
    let mut occupied = vec![false; n_cols * n_rows];
    let mut next_cell = 0usize;

    for (ci, child) in input.children.iter().enumerate() {
        let start_cell = if child.cell > 0 && child.cell < occupied.len() { child.cell }
                         else {
                             while next_cell < occupied.len() && occupied[next_cell] { next_cell += 1; }
                             next_cell
                         };
        if start_cell >= occupied.len() { break; }
        let col = start_cell % n_cols;
        let row = start_cell / n_cols;

        let span_cols = (child.col_span as usize).min(n_cols - col);
        let span_rows = (child.row_span as usize).min(n_rows - row);
        for r in 0..span_rows { for c in 0..span_cols {
            let idx = (row + r) * n_cols + (col + c);
            if idx < occupied.len() { occupied[idx] = true; }
        }}

        assignments.push(CellAssignment { child_idx: ci, col, row, col_span: span_cols as u32, row_span: span_rows as u32 });
        next_cell = start_cell + span_cols;
    }

    // Compute child rects
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
}
