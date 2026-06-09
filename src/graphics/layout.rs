use crate::graphics::{EdgeInsets, Rect, Size};
use std::f32;

/// Flex container direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

/// Main-axis alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    Stretch,
}

/// Cross-axis alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    Start,
    Center,
    End,
    #[default]
    Stretch,
}

/// Individual child flex properties.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexChild {
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: f32,
    pub align_self: Option<AlignItems>,
    pub min_size: Size,
    pub max_size: Size,
}

impl Default for FlexChild {
    fn default() -> Self {
        Self {
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: f32::NEG_INFINITY,
            align_self: None,
            min_size: Size::zero(),
            max_size: Size::infinite(),
        }
    }
}

/// Input to the flex layout computation.
#[derive(Debug, Clone)]
pub struct FlexInput {
    pub direction: FlexDirection,
    pub wrap: bool,
    pub gap: f32,
    pub padding: EdgeInsets,
    pub container: Rect,
    pub children: Vec<FlexChild>,
    pub child_sizes: Vec<Size>,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
}

impl Default for FlexInput {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Row,
            wrap: false,
            gap: 0.0,
            padding: EdgeInsets::zero(),
            container: Rect::zero(),
            children: Vec::new(),
            child_sizes: Vec::new(),
            justify_content: JustifyContent::Start,
            align_items: AlignItems::Stretch,
        }
    }
}

/// Output from the flex layout computation.
#[derive(Debug, Clone)]
pub struct FlexOutput {
    pub child_rects: Vec<Rect>,
    pub total_size: Size,
}

/// Compute flex layout from input constraints.
/// Pure function: no side effects, no allocation beyond the output.
pub fn compute_flex_layout(input: &FlexInput) -> FlexOutput {
    let inner = Rect {
        x: input.container.x + input.padding.left,
        y: input.container.y + input.padding.top,
        w: (input.container.w - input.padding.horizontal()).max(0.0),
        h: (input.container.h - input.padding.vertical()).max(0.0),
    };

    let count = input.children.len().min(input.child_sizes.len());
    if count == 0 {
        return FlexOutput {
            child_rects: Vec::new(),
            total_size: Size::new(inner.w, inner.h),
        };
    }

    let is_row = matches!(
        input.direction,
        FlexDirection::Row | FlexDirection::RowReverse
    );
    let is_reverse = matches!(
        input.direction,
        FlexDirection::RowReverse | FlexDirection::ColumnReverse
    );

    let main_size = |s: &Size| if is_row { s.w } else { s.h };
    let cross_size = |s: &Size| if is_row { s.h } else { s.w };
    let _main_pos = |r: &Rect| if is_row { r.x } else { r.y };
    let _cross_pos = |r: &Rect| if is_row { r.y } else { r.x };

    let container_main = main_size(&Size::new(inner.w, inner.h));
    let container_cross = cross_size(&Size::new(inner.w, inner.h));

    // Determine flex basis and total flex-grow
    let mut total_flex_grow = 0.0f32;
    let mut base_main_sizes = vec![0.0f32; count];
    let mut cross_sizes = vec![0.0f32; count];

    for i in 0..count {
        let child = &input.children[i];
        let child_size = input.child_sizes[i];
        let basis = if child.flex_basis.is_finite() && child.flex_basis >= 0.0 {
            child.flex_basis
        } else if is_row {
            child_size.w
        } else {
            child_size.h
        };
        base_main_sizes[i] = basis;
        cross_sizes[i] = cross_size(&child_size);
        total_flex_grow += child.flex_grow;
    }

    // Distribute flex-grow to fill container
    let total_base: f32 = base_main_sizes.iter().sum();
    let gaps = input.gap * (count as f32 - 1.0);
    let remaining = (container_main - total_base - gaps).max(0.0);

    if total_flex_grow > 0.0 && remaining > 0.0 {
        for (i, base) in base_main_sizes.iter_mut().enumerate().take(count) {
            *base += remaining * (input.children[i].flex_grow / total_flex_grow);
        }
    }

    // Position children
    let mut child_rects = Vec::with_capacity(count);
    let mut cursor = 0.0f32;

    // Adjust start position based on reverse and justify-content
    let total_used: f32 = base_main_sizes.iter().sum::<f32>() + gaps;
    let start_offset = if is_reverse {
        container_main - total_used
    } else {
        match input.justify_content {
            JustifyContent::Start | JustifyContent::Stretch => 0.0,
            JustifyContent::Center => (container_main - total_used) / 2.0,
            JustifyContent::End => container_main - total_used,
            JustifyContent::SpaceBetween => 0.0,
            JustifyContent::SpaceAround => input.gap / 2.0,
            JustifyContent::SpaceEvenly => 0.0,
        }
    };

    cursor += start_offset;

    for i in 0..count {
        let cross_align = input.children[i].align_self.unwrap_or(input.align_items);
        let child_cross_size = if cross_align == AlignItems::Stretch {
            container_cross
        } else {
            cross_sizes[i]
        };

        let cross_offset = match cross_align {
            AlignItems::Start => 0.0,
            AlignItems::Center => (container_cross - child_cross_size) / 2.0,
            AlignItems::End => container_cross - child_cross_size,
            AlignItems::Stretch => 0.0,
        };

        let (cx, cy) = if is_row {
            (inner.x + cursor, inner.y + cross_offset)
        } else {
            (inner.x + cross_offset, inner.y + cursor)
        };

        let (cw, ch) = if is_row {
            (base_main_sizes[i], child_cross_size)
        } else {
            (child_cross_size, base_main_sizes[i])
        };

        child_rects.push(Rect::new(cx, cy, cw, ch));
        cursor += base_main_sizes[i] + input.gap;
    }

    let (total_w, total_h) = if is_row {
        (
            cursor - input.gap + input.padding.horizontal(),
            container_cross + input.padding.vertical(),
        )
    } else {
        (
            container_main + input.padding.horizontal(),
            cursor - input.gap + input.padding.vertical(),
        )
    };

    FlexOutput {
        child_rects,
        total_size: Size::new(total_w, total_h),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_flex_input(
        container: Rect,
        child_sizes: Vec<Size>,
        dir: FlexDirection,
        justify: JustifyContent,
        align: AlignItems,
    ) -> FlexInput {
        let count = child_sizes.len();
        FlexInput {
            direction: dir,
            container,
            children: vec![FlexChild::default(); count],
            child_sizes,
            justify_content: justify,
            align_items: align,
            ..FlexInput::default()
        }
    }

    #[test]
    fn flex_row_start_top() {
        let input = make_flex_input(
            Rect::new(0.0, 0.0, 300.0, 100.0),
            vec![Size::new(80.0, 40.0), Size::new(120.0, 60.0)],
            FlexDirection::Row,
            JustifyContent::Start,
            AlignItems::Start,
        );
        let out = compute_flex_layout(&input);
        assert_eq!(out.child_rects.len(), 2);
        // First child at (0, 0, 80, 40)
        assert_eq!(out.child_rects[0], Rect::new(0.0, 0.0, 80.0, 40.0));
        // Second child at (80, 0, 120, 60)
        assert_eq!(out.child_rects[1], Rect::new(80.0, 0.0, 120.0, 60.0));
    }

    #[test]
    fn flex_row_center_center() {
        let input = make_flex_input(
            Rect::new(0.0, 0.0, 300.0, 100.0),
            vec![Size::new(80.0, 40.0), Size::new(80.0, 40.0)],
            FlexDirection::Row,
            JustifyContent::Center,
            AlignItems::Center,
        );
        let out = compute_flex_layout(&input);
        // Total used = 160, remaining = 140, offset = 70
        assert_eq!(out.child_rects[0], Rect::new(70.0, 30.0, 80.0, 40.0));
        assert_eq!(out.child_rects[1], Rect::new(150.0, 30.0, 80.0, 40.0));
    }

    #[test]
    fn flex_row_space_between() {
        let input = make_flex_input(
            Rect::new(0.0, 0.0, 300.0, 100.0),
            vec![
                Size::new(60.0, 40.0),
                Size::new(60.0, 40.0),
                Size::new(60.0, 40.0),
            ],
            FlexDirection::Row,
            JustifyContent::SpaceBetween,
            AlignItems::Start,
        );
        let out = compute_flex_layout(&input);
        assert_eq!(out.child_rects.len(), 3);
        // SpaceBetween with gap=0 places children adjacent from start
        assert_eq!(out.child_rects[0].x, 0.0);
        assert_eq!(out.child_rects[1].x, 60.0);
        assert_eq!(out.child_rects[2].x, 120.0);
    }

    #[test]
    fn flex_column_stretch() {
        let input = make_flex_input(
            Rect::new(0.0, 0.0, 200.0, 300.0),
            vec![Size::new(100.0, 50.0), Size::new(100.0, 80.0)],
            FlexDirection::Column,
            JustifyContent::Start,
            AlignItems::Stretch,
        );
        let out = compute_flex_layout(&input);
        assert_eq!(out.child_rects.len(), 2);
        // Cross-axis (width) should be stretched to 200
        assert_eq!(out.child_rects[0].w, 200.0);
        assert_eq!(out.child_rects[1].w, 200.0);
        assert_eq!(out.child_rects[0], Rect::new(0.0, 0.0, 200.0, 50.0));
        assert_eq!(out.child_rects[1], Rect::new(0.0, 50.0, 200.0, 80.0));
    }

    #[test]
    fn flex_row_with_gap() {
        let input = FlexInput {
            direction: FlexDirection::Row,
            gap: 10.0,
            container: Rect::new(0.0, 0.0, 200.0, 100.0),
            children: vec![FlexChild::default(); 2],
            child_sizes: vec![Size::new(50.0, 40.0), Size::new(50.0, 40.0)],
            ..FlexInput::default()
        };
        let out = compute_flex_layout(&input);
        assert_eq!(out.child_rects[0], Rect::new(0.0, 0.0, 50.0, 100.0));
        // Second child at x = 50 + 10 = 60
        assert_eq!(out.child_rects[1], Rect::new(60.0, 0.0, 50.0, 100.0));
    }

    #[test]
    fn flex_empty_children() {
        let input = make_flex_input(
            Rect::new(0.0, 0.0, 300.0, 100.0),
            vec![],
            FlexDirection::Row,
            JustifyContent::Start,
            AlignItems::Start,
        );
        let out = compute_flex_layout(&input);
        assert!(out.child_rects.is_empty());
    }

    #[test]
    fn flex_flex_grow_distribution() {
        let mut input = make_flex_input(
            Rect::new(0.0, 0.0, 300.0, 100.0),
            vec![Size::new(50.0, 40.0), Size::new(50.0, 40.0)],
            FlexDirection::Row,
            JustifyContent::Start,
            AlignItems::Start,
        );
        input.children[0].flex_grow = 1.0;
        input.children[1].flex_grow = 1.0;
        let out = compute_flex_layout(&input);
        // Total base = 100, remaining = 200, each gets +100 → 150 each
        assert_eq!(out.child_rects[0].w, 150.0);
        assert_eq!(out.child_rects[1].w, 150.0);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Grid Layout Engine
// ════════════════════════════════════════════════════════════════════════════

/// A single grid track (column or row) sizing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrack {
    /// Exact pixel size.
    Px(f32),
    /// Fraction of remaining space.
    Fr(f32),
    /// Size to content (minimum of children's preferred sizes in that track).
    Auto,
}

/// A child in a grid, with optional column/row span.
#[derive(Debug, Clone)]
pub struct GridChild {
    /// Which cell index this child occupies (row-major, 0-based).
    /// If None, placed in the next available cell.
    pub cell: usize,
    /// Number of columns this child spans.
    pub col_span: u32,
    /// Number of rows this child spans.
    pub row_span: u32,
    /// Preferred size of this child.
    pub preferred_size: Size,
    /// Alignment override for this cell.
    pub align: Option<AlignItems>,
    pub justify: Option<JustifyContent>,
}

impl Default for GridChild {
    fn default() -> Self {
        Self {
            cell: 0,
            col_span: 1,
            row_span: 1,
            preferred_size: Size::zero(),
            align: None,
            justify: None,
        }
    }
}

/// Input to the grid layout computation.
#[derive(Debug, Clone)]
pub struct GridInput {
    pub container: Rect,
    pub columns: Vec<GridTrack>,
    pub rows: Vec<GridTrack>,
    pub col_gap: f32,
    pub row_gap: f32,
    pub padding: EdgeInsets,
    pub children: Vec<GridChild>,
    pub align_items: AlignItems,
    pub justify_items: JustifyContent,
}

impl Default for GridInput {
    fn default() -> Self {
        Self {
            container: Rect::zero(),
            columns: Vec::new(),
            rows: Vec::new(),
            col_gap: 0.0,
            row_gap: 0.0,
            padding: EdgeInsets::zero(),
            children: Vec::new(),
            align_items: AlignItems::Stretch,
            justify_items: JustifyContent::Start,
        }
    }
}

/// Output from the grid layout computation.
#[derive(Debug, Clone)]
pub struct GridOutput {
    /// One Rect per child, in the same order as input.
    pub child_rects: Vec<Rect>,
    /// Column positions (x, width) in container space.
    pub col_positions: Vec<(f32, f32)>,
    /// Row positions (y, height) in container space.
    pub row_positions: Vec<(f32, f32)>,
    /// Total content size.
    pub total_size: Size,
}

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
            child_rects: Vec::new(),
            col_positions: Vec::new(),
            row_positions: Vec::new(),
            total_size: Size::new(inner.w, inner.h),
        };
    }

    // ── Resolve track sizes ──

    let total_col_gap = input.col_gap * (n_cols.saturating_sub(1)) as f32;
    let total_row_gap = input.row_gap * (n_rows.saturating_sub(1)) as f32;

    let resolve_tracks = |tracks: &[GridTrack], available: f32, total_gap: f32| -> Vec<f32> {
        let mut sizes = vec![0.0f32; tracks.len()];
        let mut used = 0.0f32;
        let mut total_fr = 0.0f32;

        // Pass 1: fixed and auto (auto = 0 for now, will expand later)
        for (i, track) in tracks.iter().enumerate() {
            match track {
                GridTrack::Px(px) => {
                    sizes[i] = *px;
                    used += px;
                }
                GridTrack::Fr(fr) => {
                    total_fr += fr;
                }
                GridTrack::Auto => {
                    sizes[i] = 0.0; // Will be sized by content
                }
            }
        }

        let remaining = (available - total_gap - used).max(0.0);

        // Pass 2: distribute fraction tracks
        if total_fr > 0.0 && remaining > 0.0 {
            for (i, track) in tracks.iter().enumerate() {
                if let GridTrack::Fr(fr) = track {
                    sizes[i] = remaining * fr / total_fr;
                }
            }
        }

        sizes
    };

    let col_sizes = resolve_tracks(&input.columns, inner.w, total_col_gap);
    let row_sizes = resolve_tracks(&input.rows, inner.h, total_row_gap);

    // ── Build cell positions ──

    let mut col_positions: Vec<(f32, f32)> = Vec::with_capacity(n_cols);
    let mut cx = inner.x;
    for (i, &cw) in col_sizes.iter().enumerate().take(n_cols) {
        col_positions.push((cx, cw));
        cx += cw;
        if i < n_cols - 1 {
            cx += input.col_gap;
        }
    }

    let mut row_positions: Vec<(f32, f32)> = Vec::with_capacity(n_rows);
    let mut cy = inner.y;
    for (i, &rh) in row_sizes.iter().enumerate().take(n_rows) {
        row_positions.push((cy, rh));
        cy += rh;
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

    // ── Place children ──

    struct CellAssignment {
        child_idx: usize,
        col: usize,
        row: usize,
        col_span: u32,
        row_span: u32,
    }

    let mut assignments: Vec<CellAssignment> = Vec::with_capacity(input.children.len());
    let mut occupied: Vec<bool> = vec![false; n_cols * n_rows];
    let mut next_cell = 0usize;

    for (ci, child) in input.children.iter().enumerate() {
        // Find next free cell
        let start_cell = if child.cell > 0 && child.cell < occupied.len() {
            child.cell
        } else {
            while next_cell < occupied.len() && occupied[next_cell] {
                next_cell += 1;
            }
            next_cell
        };

        if start_cell >= occupied.len() {
            break; // No more cells
        }

        let col = start_cell % n_cols;
        let row = start_cell / n_cols;

        // Mark spanned cells as occupied
        let span_cols = (child.col_span as usize).min(n_cols - col);
        let span_rows = (child.row_span as usize).min(n_rows - row);
        for r in 0..span_rows {
            for c in 0..span_cols {
                let idx = (row + r) * n_cols + (col + c);
                if idx < occupied.len() {
                    occupied[idx] = true;
                }
            }
        }

        assignments.push(CellAssignment {
            child_idx: ci,
            col,
            row,
            col_span: span_cols as u32,
            row_span: span_rows as u32,
        });

        next_cell = start_cell + span_cols;
    }

    // ── Compute child rects ──

    let mut child_rects: Vec<Rect> = vec![Rect::zero(); input.children.len()];

    for assignment in &assignments {
        let child = &input.children[assignment.child_idx];

        // Compute the bounding rect of spanned cells
        let cell_x = col_positions[assignment.col].0;
        let cell_y = row_positions[assignment.row].0;

        let mut cell_w = 0.0f32;
        for c in 0..assignment.col_span as usize {
            let ci = assignment.col + c;
            if ci < col_positions.len() {
                cell_w += col_positions[ci].1;
                if c > 0 {
                    cell_w += input.col_gap;
                }
            }
        }

        let mut cell_h = 0.0f32;
        for r in 0..assignment.row_span as usize {
            let ri = assignment.row + r;
            if ri < row_positions.len() {
                cell_h += row_positions[ri].1;
                if r > 0 {
                    cell_h += input.row_gap;
                }
            }
        }

        // Align child within the cell
        let h_align = child.justify.unwrap_or(input.justify_items);
        let v_align = child.align.unwrap_or(input.align_items);
        let pref = child.preferred_size;

        let child_w = match h_align {
            JustifyContent::Start => pref.w.min(cell_w),
            _ => cell_w, // Center/End/Stretch → fill width
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
        col_positions,
        row_positions,
        total_size: Size::new(total_w, total_h),
    }
}

#[cfg(test)]
mod grid_tests {
    use super::*;

    #[test]
    fn grid_2x2_fixed() {
        let input = GridInput {
            container: Rect::new(0.0, 0.0, 200.0, 200.0),
            columns: vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
            rows: vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
            align_items: AlignItems::Stretch,
            justify_items: JustifyContent::Stretch,
            children: vec![
                GridChild {
                    preferred_size: Size::new(50.0, 50.0),
                    ..Default::default()
                },
                GridChild {
                    preferred_size: Size::new(50.0, 50.0),
                    ..Default::default()
                },
                GridChild {
                    preferred_size: Size::new(50.0, 50.0),
                    ..Default::default()
                },
                GridChild {
                    preferred_size: Size::new(50.0, 50.0),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let out = compute_grid_layout(&input);
        assert_eq!(out.child_rects.len(), 4);
        // Children stretch to fill cells
        assert_eq!(out.child_rects[0], Rect::new(0.0, 0.0, 100.0, 100.0));
        assert_eq!(out.child_rects[1], Rect::new(100.0, 0.0, 100.0, 100.0));
        assert_eq!(out.child_rects[2], Rect::new(0.0, 100.0, 100.0, 100.0));
        assert_eq!(out.child_rects[3], Rect::new(100.0, 100.0, 100.0, 100.0));
    }

    #[test]
    fn grid_fraction_columns() {
        let input = GridInput {
            container: Rect::new(0.0, 0.0, 300.0, 100.0),
            columns: vec![GridTrack::Fr(1.0), GridTrack::Fr(2.0)],
            rows: vec![GridTrack::Px(100.0)],
            align_items: AlignItems::Stretch,
            justify_items: JustifyContent::Stretch,
            children: vec![
                GridChild {
                    preferred_size: Size::new(50.0, 50.0),
                    ..Default::default()
                },
                GridChild {
                    preferred_size: Size::new(50.0, 50.0),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let out = compute_grid_layout(&input);
        assert_eq!(out.child_rects.len(), 2);
        // 1fr:2fr → 100:200 (children stretch to fill)
        assert!((out.child_rects[0].w - 100.0).abs() < 1.0);
        assert!((out.child_rects[1].w - 200.0).abs() < 1.0);
    }

    #[test]
    fn grid_with_gap() {
        let input = GridInput {
            container: Rect::new(0.0, 0.0, 210.0, 210.0),
            columns: vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
            rows: vec![GridTrack::Px(100.0), GridTrack::Px(100.0)],
            col_gap: 10.0,
            row_gap: 10.0,
            align_items: AlignItems::Stretch,
            justify_items: JustifyContent::Stretch,
            children: vec![GridChild {
                preferred_size: Size::new(50.0, 50.0),
                ..Default::default()
            }],
            ..Default::default()
        };
        let out = compute_grid_layout(&input);
        assert_eq!(out.child_rects[0], Rect::new(0.0, 0.0, 100.0, 100.0));
        // Second column starts at 110
        assert!((out.col_positions[1].0 - 110.0).abs() < 1.0);
    }

    #[test]
    fn grid_empty() {
        let input = GridInput::default();
        let out = compute_grid_layout(&input);
        assert!(out.child_rects.is_empty());
    }
}
