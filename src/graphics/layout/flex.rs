use super::*;

/// Compute flex layout from input constraints.
///
/// Pure function with no side effects. Handles all justify-content modes
/// (Start, Center, End, SpaceBetween, SpaceAround, SpaceEvenly, Stretch),
/// cross-axis alignment via AlignItems and per-child align_self, and
/// flex-grow distribution.
pub fn compute_flex_layout(input: &FlexInput) -> FlexOutput {
    let inner = Rect {
        x: input.container.x + input.padding.left,
        y: input.container.y + input.padding.top,
        w: (input.container.w - input.padding.horizontal()).max(0.0),
        h: (input.container.h - input.padding.vertical()).max(0.0),
    };

    let count = input.children.len().min(input.child_sizes.len());
    if count == 0 {
        return FlexOutput { child_rects: Vec::new(), total_size: Size::new(inner.w, inner.h) };
    }

    let is_row = matches!(input.direction, FlexDirection::Row | FlexDirection::RowReverse);
    let is_reverse = matches!(input.direction, FlexDirection::RowReverse | FlexDirection::ColumnReverse);

    let main_size = |s: &Size| if is_row { s.w } else { s.h };
    let cross_size = |s: &Size| if is_row { s.h } else { s.w };

    let container_main = main_size(&Size::new(inner.w, inner.h));
    let container_cross = cross_size(&Size::new(inner.w, inner.h));

    // Phase 1: determine flex basis and cross sizes
    let mut total_flex_grow = 0.0f32;
    let mut base_main_sizes = vec![0.0f32; count];
    let mut cross_sizes = vec![0.0f32; count];

    for i in 0..count {
        let child = &input.children[i];
        let child_size = input.child_sizes[i];
        let basis = if child.flex_basis.is_finite() && child.flex_basis >= 0.0 {
            child.flex_basis
        } else if is_row { child_size.w } else { child_size.h };
        base_main_sizes[i] = basis;
        cross_sizes[i] = cross_size(&child_size);
        total_flex_grow += child.flex_grow;
    }

    // Phase 2: distribute flex-grow
    let total_base: f32 = base_main_sizes.iter().sum();
    let gaps = input.gap * (count as f32 - 1.0);
    let overflow = total_base + gaps - container_main;

    if overflow > 0.0 {
        // Shrink phase: 按 flex_shrink 比例压缩溢出的子节点
        let total_shrink_weight: f32 = base_main_sizes.iter()
            .zip(input.children.iter())
            .map(|(&sz, ch)| ch.flex_shrink * sz)
            .sum();
        if total_shrink_weight > 0.0 {
            let mut shrunk = 0.0f32;
            for (i, base) in base_main_sizes.iter_mut().enumerate().take(count) {
                if *base <= 0.0 { continue; }
                let weight = input.children[i].flex_shrink * *base / total_shrink_weight;
                let reduction = (overflow * weight).min(*base - 1.0); // 至少保留1px
                *base -= reduction;
                shrunk += reduction;
            }
        }
    }
    let total_base: f32 = base_main_sizes.iter().sum();
    let mut remaining = (container_main - total_base - gaps).max(0.0);

    if total_flex_grow > 0.0 && remaining > 0.0 {
        for (i, base) in base_main_sizes.iter_mut().enumerate().take(count) {
            *base += remaining * (input.children[i].flex_grow / total_flex_grow);
        }
    }

    // Phase 3: recompute remaining, handle justify modes
    let total_after: f32 = base_main_sizes.iter().sum();
    remaining = (container_main - total_after - gaps).max(0.0);

    let (effective_gap, start_offset) = if remaining > 0.0 {
        let gap = match input.justify_content {
            JustifyContent::SpaceBetween => {
                if count <= 1 { input.gap } else { input.gap + remaining / (count - 1) as f32 }
            }
            JustifyContent::SpaceAround => input.gap + remaining / count as f32,
            JustifyContent::SpaceEvenly => input.gap + remaining / (count + 1) as f32,
            _ => input.gap,
        };
        let offset = if is_reverse { remaining }
        else {
            match input.justify_content {
                JustifyContent::Center => remaining * 0.5,
                JustifyContent::End => remaining,
                JustifyContent::SpaceAround => remaining * 0.5 / count as f32,
                JustifyContent::SpaceEvenly => remaining / (count + 1) as f32,
                _ => 0.0,
            }
        };
        (gap, offset)
    } else {
        let offset = match input.justify_content {
            JustifyContent::Center => (container_main - total_after - gaps).max(0.0) * 0.5,
            JustifyContent::End => (container_main - total_after - gaps).max(0.0),
            _ => 0.0,
        };
        (input.gap, offset)
    };

    // Phase 4: position children
    let mut child_rects = Vec::with_capacity(count);
    let mut cursor = if is_reverse { container_main - start_offset - total_after - gaps }
                     else { start_offset };

    for i in 0..count {
        let cross_align = input.children[i].align_self.unwrap_or(input.align_items);
        let child_cross_size = if cross_align == AlignItems::Stretch { container_cross } else { cross_sizes[i] };

        let cross_offset = match cross_align {
            AlignItems::Start => 0.0,
            AlignItems::Center => (container_cross - child_cross_size) / 2.0,
            AlignItems::End => container_cross - child_cross_size,
            AlignItems::Stretch => 0.0,
        };

        let (cx, cy) = if is_row { (inner.x + cursor, inner.y + cross_offset) }
                       else { (inner.x + cross_offset, inner.y + cursor) };
        let (cw, ch) = if is_row { (base_main_sizes[i], child_cross_size) }
                       else { (child_cross_size, base_main_sizes[i]) };

        child_rects.push(Rect::new(cx, cy, cw, ch));
        cursor += if is_reverse { -(base_main_sizes[i] + effective_gap) }
                  else { base_main_sizes[i] + effective_gap };
    }

    let (total_w, total_h) = if is_row {
        let max_cross = cross_sizes.iter().cloned().fold(0.0, f32::max);
        (cursor.max(0.0) + input.padding.horizontal(), (max_cross + input.padding.vertical()).max(container_cross))
    } else {
        let max_cross = cross_sizes.iter().cloned().fold(0.0, f32::max);
        ((max_cross + input.padding.horizontal()).max(container_cross), cursor.max(0.0) + input.padding.vertical())
    };

    FlexOutput { child_rects, total_size: Size::new(total_w, total_h) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(c: Rect, sizes: Vec<Size>, dir: FlexDirection, j: JustifyContent, a: AlignItems) -> FlexInput {
        FlexInput {
            direction: dir, container: c,
            children: vec![FlexChild::default(); sizes.len()],
            child_sizes: sizes, justify_content: j, align_items: a,
            ..FlexInput::default()
        }
    }

    #[test] fn row_start_top() {
        let o = compute_flex_layout(&make_input(Rect::new(0.0,0.0,300.0,100.0),
            vec![Size::new(80.0,40.0),Size::new(120.0,60.0)],
            FlexDirection::Row, JustifyContent::Start, AlignItems::Start));
        assert_eq!(o.child_rects[0], Rect::new(0.0,0.0,80.0,40.0));
        assert_eq!(o.child_rects[1], Rect::new(80.0,0.0,120.0,60.0));
    }
    #[test] fn row_center_center() {
        let o = compute_flex_layout(&make_input(Rect::new(0.0,0.0,300.0,100.0),
            vec![Size::new(80.0,40.0),Size::new(80.0,40.0)],
            FlexDirection::Row, JustifyContent::Center, AlignItems::Center));
        assert_eq!(o.child_rects[0], Rect::new(70.0,30.0,80.0,40.0));
        assert_eq!(o.child_rects[1], Rect::new(150.0,30.0,80.0,40.0));
    }
    #[test] fn row_space_between() {
        let o = compute_flex_layout(&make_input(Rect::new(0.0,0.0,300.0,100.0),
            vec![Size::new(60.0,40.0),Size::new(60.0,40.0),Size::new(60.0,40.0)],
            FlexDirection::Row, JustifyContent::SpaceBetween, AlignItems::Start));
        assert_eq!(o.child_rects[0].x, 0.0); assert_eq!(o.child_rects[1].x, 120.0); assert_eq!(o.child_rects[2].x, 240.0);
    }
    #[test] fn column_stretch() {
        let o = compute_flex_layout(&make_input(Rect::new(0.0,0.0,200.0,300.0),
            vec![Size::new(100.0,50.0),Size::new(100.0,80.0)],
            FlexDirection::Column, JustifyContent::Start, AlignItems::Stretch));
        assert_eq!(o.child_rects[0], Rect::new(0.0,0.0,200.0,50.0));
        assert_eq!(o.child_rects[1], Rect::new(0.0,50.0,200.0,80.0));
    }
    #[test] fn row_with_gap() {
        let o = compute_flex_layout(&FlexInput {
            direction: FlexDirection::Row, gap: 10.0, container: Rect::new(0.0,0.0,200.0,100.0),
            children: vec![FlexChild::default();2],
            child_sizes: vec![Size::new(50.0,40.0),Size::new(50.0,40.0)], ..FlexInput::default()
        });
        assert_eq!(o.child_rects[0], Rect::new(0.0,0.0,50.0,100.0));
        assert_eq!(o.child_rects[1], Rect::new(60.0,0.0,50.0,100.0));
    }
    #[test] fn flex_grow_distribution() {
        let mut i = make_input(Rect::new(0.0,0.0,300.0,100.0),
            vec![Size::new(50.0,40.0),Size::new(50.0,40.0)],
            FlexDirection::Row, JustifyContent::Start, AlignItems::Start);
        i.children[0].flex_grow = 1.0; i.children[1].flex_grow = 1.0;
        assert_eq!(compute_flex_layout(&i).child_rects[0].w, 150.0);
    }
    #[test] fn empty_children() { assert!(compute_flex_layout(&make_input(Rect::new(0.0,0.0,300.0,100.0),
        vec![], FlexDirection::Row, JustifyContent::Start, AlignItems::Start)).child_rects.is_empty()); }
    #[test] fn row_space_around() {
        let o = compute_flex_layout(&make_input(Rect::new(0.0,0.0,300.0,100.0),
            vec![Size::new(60.0,40.0),Size::new(60.0,40.0)],
            FlexDirection::Row, JustifyContent::SpaceAround, AlignItems::Start));
        assert!((o.child_rects[0].x - 45.0).abs() < 1.0);
        assert!((o.child_rects[1].x - 195.0).abs() < 1.0);
    }
    #[test] fn row_space_evenly() {
        let o = compute_flex_layout(&make_input(Rect::new(0.0,0.0,300.0,100.0),
            vec![Size::new(60.0,40.0),Size::new(60.0,40.0)],
            FlexDirection::Row, JustifyContent::SpaceEvenly, AlignItems::Start));
        assert!((o.child_rects[0].x - 60.0).abs() < 1.0);
        assert!((o.child_rects[1].x - 180.0).abs() < 1.0);
    }
    #[test] fn row_end() {
        let o = compute_flex_layout(&make_input(Rect::new(0.0,0.0,300.0,100.0),
            vec![Size::new(80.0,40.0),Size::new(80.0,40.0)],
            FlexDirection::Row, JustifyContent::End, AlignItems::Start));
        assert_eq!(o.child_rects[0].x, 140.0); assert_eq!(o.child_rects[1].x, 220.0);
    }
    #[test] fn row_reverse_basic() {
        let o = compute_flex_layout(&make_input(Rect::new(0.0,0.0,300.0,100.0),
            vec![Size::new(80.0,40.0),Size::new(80.0,40.0)],
            FlexDirection::RowReverse, JustifyContent::Start, AlignItems::Start));
        assert!(o.child_rects[0].x > o.child_rects[1].x);
    }
    #[test] fn column_reverse_basic() {
        let o = compute_flex_layout(&make_input(Rect::new(0.0,0.0,200.0,300.0),
            vec![Size::new(100.0,50.0),Size::new(100.0,80.0)],
            FlexDirection::ColumnReverse, JustifyContent::Start, AlignItems::Start));
        assert!(o.child_rects[0].y > o.child_rects[1].y);
    }
}
