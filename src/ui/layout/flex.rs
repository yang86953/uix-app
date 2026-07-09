use super::*;

/// Compute flex layout from input constraints.
///
/// Pure function with no side effects. Handles all justify-content modes,
/// cross-axis alignment via AlignItems and per-child align_self,
/// flex-grow/shrink distribution, flex-basis, min_size/max_size constraints,
/// and multi-line wrapping.
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

    let container_main = main_size(&Size::new(inner.w, inner.h));
    let container_cross = cross_size(&Size::new(inner.w, inner.h));
    let intrinsic_main = input.intrinsic_main;

    // Phase 1: determine flex basis and cross sizes
    let mut base_main_sizes = vec![0.0f32; count];
    let mut cross_sizes = vec![0.0f32; count];
    let mut total_flex_grow = 0.0f32;

    for i in 0..count {
        let child = &input.children[i];
        let child_size = input.child_sizes[i];
        let basis = match child.flex_basis {
            Some(v) if v >= 0.0 => v,
            _ => main_size(&child_size),
        };
        base_main_sizes[i] = basis;
        cross_sizes[i] = cross_size(&child_size);
        total_flex_grow += child.flex_grow;
    }

    if input.wrap {
        compute_wrapped(
            input,
            &inner,
            is_row,
            is_reverse,
            container_main,
            container_cross,
            &mut base_main_sizes,
            &mut cross_sizes,
            total_flex_grow,
        )
    } else {
        compute_single_line(
            input,
            &inner,
            is_row,
            is_reverse,
            container_main,
            container_cross,
            intrinsic_main,
            &mut base_main_sizes,
            &mut cross_sizes,
            total_flex_grow,
        )
    }
}

fn clamp_sizes(base: &mut [f32], cross: &mut [f32], children: &[FlexChild], is_row: bool) {
    for i in 0..base.len() {
        let child = &children[i];
        let sz = make_size_from(is_row, base[i], cross[i]);

        let clamped = Size::new(
            sz.w.max(child.min_size.w).min(child.max_size.w),
            sz.h.max(child.min_size.h).min(child.max_size.h),
        );

        let new_main = if is_row { clamped.w } else { clamped.h };
        let new_cross = if is_row { clamped.h } else { clamped.w };

        if (base[i] - new_main).abs() > 0.001 || (cross[i] - new_cross).abs() > 0.001 {
            base[i] = new_main;
            cross[i] = new_cross;
        }
    }
}

fn make_size_from(is_row: bool, main: f32, cross: f32) -> Size {
    if is_row {
        Size::new(main, cross)
    } else {
        Size::new(cross, main)
    }
}

fn child_margin(input: &FlexInput, index: usize) -> EdgeInsets {
    input.child_margins.get(index).copied().unwrap_or_default()
}

fn margin_main(margin: EdgeInsets, is_row: bool) -> f32 {
    if is_row {
        margin.horizontal()
    } else {
        margin.vertical()
    }
}

fn margin_cross(margin: EdgeInsets, is_row: bool) -> f32 {
    if is_row {
        margin.vertical()
    } else {
        margin.horizontal()
    }
}

fn margin_main_start(margin: EdgeInsets, is_row: bool, is_reverse: bool) -> f32 {
    match (is_row, is_reverse) {
        (true, false) => margin.left,
        (true, true) => margin.right,
        (false, false) => margin.top,
        (false, true) => margin.bottom,
    }
}

fn margin_cross_start(margin: EdgeInsets, is_row: bool) -> f32 {
    if is_row {
        margin.top
    } else {
        margin.left
    }
}

fn distribute_flex_grow(
    base: &mut [f32],
    remaining: f32,
    children: &[FlexChild],
    total_flex_grow: f32,
) {
    if total_flex_grow > 0.0 && remaining > 0.0 {
        for i in 0..base.len() {
            base[i] += remaining * (children[i].flex_grow / total_flex_grow);
        }
    }
}

fn distribute_shrink(base: &mut [f32], overflow: f32, children: &[FlexChild]) {
    let total_shrink_weight: f32 = base
        .iter()
        .zip(children.iter())
        .map(|(&sz, ch)| ch.flex_shrink * sz)
        .sum();
    if total_shrink_weight > 0.0 {
        for (i, b) in base.iter_mut().enumerate() {
            if *b <= 0.0 {
                continue;
            }
            let weight = children[i].flex_shrink * *b / total_shrink_weight;
            let reduction = (overflow * weight).min(*b);
            *b -= reduction;
        }
    }
}

fn compute_justify(
    remaining: f32,
    count: usize,
    gap: f32,
    justify: JustifyContent,
    is_reverse: bool,
) -> (f32, f32) {
    if remaining > 0.0 {
        let new_gap = match justify {
            JustifyContent::SpaceBetween => {
                if count <= 1 {
                    gap
                } else {
                    gap + remaining / (count - 1) as f32
                }
            }
            JustifyContent::SpaceAround => gap + remaining / count as f32,
            JustifyContent::SpaceEvenly => gap + remaining / (count + 1) as f32,
            _ => gap,
        };
        let offset = if is_reverse {
            remaining
        } else {
            match justify {
                JustifyContent::Center => remaining * 0.5,
                JustifyContent::End => remaining,
                JustifyContent::SpaceAround => remaining * 0.5 / count as f32,
                JustifyContent::SpaceEvenly => remaining / (count + 1) as f32,
                _ => 0.0,
            }
        };
        (new_gap, offset)
    } else {
        let offset = match justify {
            JustifyContent::Center => remaining * 0.5,
            JustifyContent::End => remaining,
            _ => 0.0,
        };
        (gap, offset)
    }
}

/// Single-line (no wrap) flex layout.
fn compute_single_line(
    input: &FlexInput,
    inner: &Rect,
    is_row: bool,
    is_reverse: bool,
    container_main: f32,
    container_cross: f32,
    intrinsic_main: bool,
    base_main_sizes: &mut [f32],
    cross_sizes: &mut [f32],
    total_flex_grow: f32,
) -> FlexOutput {
    let count = base_main_sizes.len();
    let bootstrap_main = container_main <= 1.0;
    let skip_shrink = intrinsic_main || bootstrap_main;
    let max_child_cross = cross_sizes.iter().cloned().fold(0.0, f32::max);
    let effective_cross = if container_cross > 0.0 {
        container_cross
    } else {
        max_child_cross
    };

    // Phase 2: distribute flex-grow/shrink
    let total_margin_main: f32 = (0..count)
        .map(|i| margin_main(child_margin(input, i), is_row))
        .sum();
    let total_base: f32 = base_main_sizes.iter().sum::<f32>() + total_margin_main;
    let gaps = input.gap * (count as f32 - 1.0);
    let overflow = if skip_shrink {
        0.0
    } else {
        (total_base + gaps - container_main).max(0.0)
    };

    if overflow > 0.0 {
        distribute_shrink(base_main_sizes, overflow, &input.children);
    }
    let total_after: f32 = base_main_sizes.iter().sum::<f32>() + total_margin_main;
    let mut remaining = if container_main > 0.0 {
        (container_main - total_after - gaps).max(0.0)
    } else {
        0.0
    };
    distribute_flex_grow(base_main_sizes, remaining, &input.children, total_flex_grow);

    // Apply Stretch justify-content: distribute remaining space as growth
    let mut total_after: f32 = base_main_sizes.iter().sum::<f32>() + total_margin_main;
    remaining = if container_main > 0.0 {
        (container_main - total_after - gaps).max(0.0)
    } else {
        0.0
    };
    if input.justify_content == JustifyContent::Stretch && remaining > 0.0 {
        let extra = remaining / count as f32;
        for b in base_main_sizes.iter_mut() {
            *b += extra;
        }
        total_after = base_main_sizes.iter().sum();
    }

    // Clamp to min/max
    clamp_sizes(base_main_sizes, cross_sizes, &input.children, is_row);

    // Redistribute: space freed by max_size clamping is given back
    // to children with remaining flex_grow capacity. Loop up to 3 rounds
    // to handle cascading clamp effects until all space is consumed.
    for _round in 0..3 {
        let current_total = base_main_sizes.iter().sum::<f32>() + total_margin_main;
        let leftover = if container_main > 0.0 {
            (container_main - current_total - gaps).max(0.0)
        } else {
            0.0
        };
        if leftover > 0.0 && total_flex_grow > 0.0 {
            distribute_flex_grow(base_main_sizes, leftover, &input.children, total_flex_grow);
            clamp_sizes(base_main_sizes, cross_sizes, &input.children, is_row);
        } else {
            break;
        }
    }

    // Phase 3: justify-content positioning
    // Reuse the most recent total to avoid an extra sum() traversal
    let total_final = base_main_sizes.iter().sum::<f32>() + total_margin_main;
    remaining = if container_main > 0.0 {
        (container_main - total_final - gaps).max(0.0)
    } else {
        0.0
    };
    let (effective_gap, start_offset) = compute_justify(
        remaining,
        count,
        input.gap,
        input.justify_content,
        is_reverse,
    );

    // Phase 4: position children
    let mut child_rects = Vec::with_capacity(count);
    let mut cursor = if is_reverse {
        container_main - start_offset - total_after - gaps
    } else {
        start_offset
    };

    for i in 0..count {
        let margin = child_margin(input, i);
        let cross_align = input.children[i].align_self.unwrap_or(input.align_items);
        let child_cross_size = if cross_align == AlignItems::Stretch {
            (effective_cross - margin_cross(margin, is_row))
                .max(cross_sizes[i])
                .max(0.0)
        } else {
            cross_sizes[i]
        };

        let cross_offset = match cross_align {
            AlignItems::Start => 0.0,
            AlignItems::Center => (effective_cross - child_cross_size) / 2.0,
            AlignItems::End => effective_cross - child_cross_size,
            AlignItems::Stretch => 0.0,
        };

        let main_start = margin_main_start(margin, is_row, is_reverse);
        let cross_start = margin_cross_start(margin, is_row);
        let (cx, cy) = if is_row {
            (
                inner.x + cursor + main_start,
                inner.y + cross_offset + cross_start,
            )
        } else {
            (
                inner.x + cross_offset + cross_start,
                inner.y + cursor + main_start,
            )
        };
        let (cw, ch) = if is_row {
            (base_main_sizes[i], child_cross_size)
        } else {
            (child_cross_size, base_main_sizes[i])
        };

        child_rects.push(Rect::new(cx, cy, cw, ch));
        let occupied_main = base_main_sizes[i] + margin_main(margin, is_row);
        cursor += if is_reverse {
            -(occupied_main + effective_gap)
        } else {
            occupied_main + effective_gap
        };
    }

    // total_size：bootstrap/无确定主轴时用子项撑开；否则占满父级分配的主轴空间。
    let main_total = if intrinsic_main && bootstrap_main {
        total_final + gaps
    } else if is_row {
        inner.w
    } else {
        inner.h
    };
    let (total_w, total_h) = if is_row {
        let cross_total = effective_cross.max(max_child_cross);
        (
            main_total + input.padding.horizontal(),
            cross_total + input.padding.vertical(),
        )
    } else {
        let cross_total = effective_cross.max(max_child_cross);
        (
            cross_total + input.padding.horizontal(),
            main_total + input.padding.vertical(),
        )
    };

    FlexOutput {
        child_rects,
        total_size: Size::new(total_w, total_h),
    }
}

/// Multi-line (wrapping) flex layout.
fn compute_wrapped(
    input: &FlexInput,
    inner: &Rect,
    is_row: bool,
    is_reverse: bool,
    container_main: f32,
    container_cross: f32,
    base_main_sizes: &mut [f32],
    cross_sizes: &mut [f32],
    _total_flex_grow: f32,
) -> FlexOutput {
    let count = base_main_sizes.len();

    // Build lines: each line is a range of child indices
    struct Line {
        start: usize,
        end: usize,
    }
    let mut lines: Vec<Line> = Vec::new();
    let mut line_start = 0usize;
    let mut line_main = 0.0f32;

    for i in 0..count {
        let child_main = base_main_sizes[i] + margin_main(child_margin(input, i), is_row);
        let item_gap = if i > line_start { input.gap } else { 0.0 };

        if line_main + item_gap + child_main > container_main && line_main > 0.0 {
            lines.push(Line {
                start: line_start,
                end: i,
            });
            line_start = i;
            line_main = child_main;
        } else {
            line_main += item_gap + child_main;
        }
    }
    if line_start < count {
        lines.push(Line {
            start: line_start,
            end: count,
        });
    }

    if lines.is_empty() {
        return FlexOutput {
            child_rects: Vec::new(),
            total_size: Size::new(inner.w, inner.h),
        };
    }

    // Track cross-axis position for each line
    let line_gap = input.gap;
    let mut line_cross_positions = Vec::with_capacity(lines.len());
    let mut line_max_cross = Vec::with_capacity(lines.len());
    let mut cursor_cross = 0.0f32;

    for line in &lines {
        let line_count = line.end - line.start;
        let line_margin_main: f32 = (line.start..line.end)
            .map(|i| margin_main(child_margin(input, i), is_row))
            .sum();
        let line_base: f32 =
            base_main_sizes[line.start..line.end].iter().sum::<f32>() + line_margin_main;
        let line_gaps = input.gap * (line_count as f32 - 1.0).max(0.0);
        let overflow = line_base + line_gaps - container_main;

        // Shrink within line
        if overflow > 0.0 {
            distribute_shrink(
                &mut base_main_sizes[line.start..line.end],
                overflow,
                &input.children[line.start..line.end],
            );
        }

        // Grow within line
        let remaining = (container_main
            - base_main_sizes[line.start..line.end].iter().sum::<f32>()
            - line_margin_main
            - line_gaps)
            .max(0.0);
        let line_grow: f32 = input.children[line.start..line.end]
            .iter()
            .map(|c| c.flex_grow)
            .sum();
        distribute_flex_grow(
            &mut base_main_sizes[line.start..line.end],
            remaining,
            &input.children[line.start..line.end],
            line_grow,
        );

        // Apply Stretch
        let total_after: f32 =
            base_main_sizes[line.start..line.end].iter().sum::<f32>() + line_margin_main;
        let remaining2 = (container_main - total_after - line_gaps).max(0.0);
        if input.justify_content == JustifyContent::Stretch && remaining2 > 0.0 {
            let extra = remaining2 / line_count as f32;
            for b in base_main_sizes[line.start..line.end].iter_mut() {
                *b += extra;
            }
        }

        // Compute cross size for this line
        let max_cross = (line.start..line.end)
            .map(|i| cross_sizes[i] + margin_cross(child_margin(input, i), is_row))
            .fold(0.0, f32::max);
        line_cross_positions.push(cursor_cross);
        line_max_cross.push(max_cross);
        cursor_cross += max_cross + line_gap;
    }

    // Clamp to min/max (across all children)
    clamp_sizes(base_main_sizes, cross_sizes, &input.children, is_row);

    // Position children line by line
    let mut child_rects = vec![Rect::zero(); count];

    // 交叉轴总尺寸：所有行/列的交叉轴尺寸 + 行间/列间 gap
    // cursor_cross 已累加每行交叉轴 + line_gap，减去最后一个多余的 gap
    let total_cross = (cursor_cross - line_gap).max(0.0);
    let cross_align = input.align_items;
    let cross_start_offset = match cross_align {
        AlignItems::Center => (container_cross - total_cross) * 0.5,
        AlignItems::End => (container_cross - total_cross).max(0.0),
        _ => 0.0,
    };

    for (li, line) in lines.iter().enumerate() {
        let line_count = line.end - line.start;
        let line_gaps_total = input.gap * (line_count as f32 - 1.0).max(0.0);
        let line_margin_main: f32 = (line.start..line.end)
            .map(|i| margin_main(child_margin(input, i), is_row))
            .sum();
        let total_line_main: f32 =
            base_main_sizes[line.start..line.end].iter().sum::<f32>() + line_margin_main;
        let remaining = (container_main - total_line_main - line_gaps_total).max(0.0);
        let (effective_gap, start_offset) = compute_justify(
            remaining,
            line_count,
            input.gap,
            input.justify_content,
            is_reverse,
        );

        let mut cursor_main = if is_reverse {
            container_main - start_offset - total_line_main - line_gaps_total
        } else {
            start_offset
        };

        let line_cross_base = line_cross_positions[li] + cross_start_offset;

        for i in line.start..line.end {
            let margin = child_margin(input, i);
            let cross_align = input.children[i].align_self.unwrap_or(input.align_items);
            let child_cross_size = if cross_align == AlignItems::Stretch {
                (line_max_cross[li] - margin_cross(margin, is_row)).max(0.0)
            } else {
                cross_sizes[i]
            };

            let cross_offset = match cross_align {
                AlignItems::Start => 0.0,
                AlignItems::Center => (line_max_cross[li] - child_cross_size) / 2.0,
                AlignItems::End => line_max_cross[li] - child_cross_size,
                AlignItems::Stretch => 0.0,
            };

            let (cx, cy) = if is_row {
                (
                    inner.x + cursor_main + margin_main_start(margin, is_row, is_reverse),
                    inner.y + line_cross_base + cross_offset + margin_cross_start(margin, is_row),
                )
            } else {
                (
                    inner.x + line_cross_base + cross_offset + margin_cross_start(margin, is_row),
                    inner.y + cursor_main + margin_main_start(margin, is_row, is_reverse),
                )
            };
            let (cw, ch) = if is_row {
                (base_main_sizes[i], child_cross_size)
            } else {
                (child_cross_size, base_main_sizes[i])
            };

            child_rects[i] = Rect::new(cx, cy, cw, ch);
            let occupied_main = base_main_sizes[i] + margin_main(margin, is_row);
            cursor_main += if is_reverse {
                -(occupied_main + effective_gap)
            } else {
                occupied_main + effective_gap
            };
        }
    }

    // Total size
    let (total_w, total_h) = if is_row {
        let max_cross = if lines.len() > 1 {
            line_cross_positions
                .last()
                .zip(line_max_cross.last())
                .map(|(&p, &s)| p + s)
                .unwrap_or(0.0)
        } else {
            cross_sizes.iter().cloned().fold(0.0, f32::max)
        };
        (
            container_main.max(0.0) + input.padding.horizontal(),
            (max_cross + input.padding.vertical()).max(container_cross),
        )
    } else {
        let max_cross = if lines.len() > 1 {
            line_cross_positions
                .last()
                .zip(line_max_cross.last())
                .map(|(&p, &s)| p + s)
                .unwrap_or(0.0)
        } else {
            cross_sizes.iter().cloned().fold(0.0, f32::max)
        };
        (
            (max_cross + input.padding.horizontal()).max(container_cross),
            container_main.max(0.0) + input.padding.vertical(),
        )
    };

    FlexOutput {
        child_rects,
        total_size: Size::new(total_w, total_h),
    }
}
