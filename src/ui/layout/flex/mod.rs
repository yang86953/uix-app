use super::*;

// 把主轴分布辅助收敛到独立小模块，避免核心求解文件越过行数门禁。
mod justify;
// 继续向公开布局适配层暴露原有 crate 内共享入口。
pub(super) use justify::compute_justify;
// 把尺寸边界归一规则收敛到独立模块，保持求解主流程聚焦。
mod sizing;
// 在当前求解器内复用统一的尺寸边界归一入口。
use sizing::normalized_axis_bounds;

/// Compute flex layout from input constraints.
///
/// Pure function with no side effects. Handles all justify-content modes,
/// cross-axis alignment via AlignItems and per-child align_self,
/// flex-grow/shrink distribution, flex-basis, min_size/max_size constraints,
/// and multi-line wrapping.
#[cfg(test)]
pub(crate) fn compute_flex_layout(input: &FlexInput<'_>) -> FlexOutput {
    // 兼容独立求解调用方：临时工作区的所有权随返回结果移交。
    let mut scratch = FlexComputeScratch::default();
    let _ = compute_flex_layout_into(input, &mut scratch);
    FlexOutput {
        child_rects: std::mem::take(&mut scratch.child_rects),
    }
}

/// 把 Flex 结果写入调用方持有的工作区，供真实布局帧跨容器复用。
pub(crate) fn compute_flex_layout_into(
    input: &FlexInput<'_>,
    scratch: &mut FlexComputeScratch,
) -> Size {
    let inner = Rect {
        x: input.container.x + input.padding.left,
        y: input.container.y + input.padding.top,
        w: (input.container.w - input.padding.horizontal()).max(0.0),
        h: (input.container.h - input.padding.vertical()).max(0.0),
    };

    let count = input.children.len();
    if count == 0 {
        scratch.child_rects.clear();
        return Size::new(inner.w, inner.h);
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
    scratch.base_main_sizes.clear();
    scratch.base_main_sizes.resize(count, 0.0);
    scratch.cross_sizes.clear();
    scratch.cross_sizes.resize(count, 0.0);
    let FlexComputeScratch {
        base_main_sizes,
        cross_sizes,
        lines,
        line_cross_positions,
        line_max_cross,
        child_rects,
    } = scratch;

    for ((base_main, cross), child) in base_main_sizes
        .iter_mut()
        .zip(cross_sizes.iter_mut())
        .zip(input.children)
    {
        let basis = match child.flex_basis {
            // 显式 basis 只接受有限、非负且不是无界哨兵的实际尺寸。
            Some(v) if v.is_finite() && v.abs() < f32::MAX && v >= 0.0 => v,
            _ => finite_non_negative(main_size(&child.measured_size)),
        };
        *base_main = basis;
        *cross = finite_non_negative(cross_size(&child.measured_size));
    }

    // 在分行和弹性分配前落实每个子项的主轴与交叉轴上下限。
    clamp_sizes(base_main_sizes, cross_sizes, input.children, is_row);

    if input.wrap {
        compute_wrapped(
            input,
            &inner,
            is_row,
            is_reverse,
            container_main,
            container_cross,
            base_main_sizes,
            cross_sizes,
            lines,
            line_cross_positions,
            line_max_cross,
            child_rects,
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
            base_main_sizes,
            cross_sizes,
            child_rects,
        )
    }
}

// 读取子项在当前主轴上的尺寸上下限。
fn child_main_bounds(child: &FlexChild, is_row: bool) -> (f32, f32) {
    // 水平主轴读取宽度约束，垂直主轴读取高度约束。
    if is_row {
        // 归一宽度上下限。
        normalized_axis_bounds(child.min_size.w, child.max_size.w)
    } else {
        // 归一高度上下限。
        normalized_axis_bounds(child.min_size.h, child.max_size.h)
    }
}

// 读取子项在当前交叉轴上的尺寸上下限。
fn child_cross_bounds(child: &FlexChild, is_row: bool) -> (f32, f32) {
    // 水平主轴的交叉轴为高度，垂直主轴的交叉轴为宽度。
    if is_row {
        // 归一高度上下限。
        normalized_axis_bounds(child.min_size.h, child.max_size.h)
    } else {
        // 归一宽度上下限。
        normalized_axis_bounds(child.min_size.w, child.max_size.w)
    }
}

// 在弹性分配前统一落实子项主轴与交叉轴约束。
fn clamp_sizes(base: &mut [f32], cross: &mut [f32], children: &[FlexChild], is_row: bool) {
    // 按相同索引遍历主轴、交叉轴与子项约束。
    for ((main, cross), child) in base.iter_mut().zip(cross).zip(children) {
        // 读取当前主轴的有序上下限。
        let (main_min, main_max) = child_main_bounds(child, is_row);
        // 读取当前交叉轴的有序上下限。
        let (cross_min, cross_max) = child_cross_bounds(child, is_row);
        // 主轴尺寸先满足最小值再满足最大值。
        *main = finite_non_negative(*main).max(main_min).min(main_max);
        // 交叉轴尺寸采用相同约束规则。
        *cross = finite_non_negative(*cross).max(cross_min).min(cross_max);
    }
}

fn child_margin(input: &FlexInput<'_>, index: usize) -> EdgeInsets {
    let margin = input.children[index].margin;
    EdgeInsets::new(
        finite_or_zero(margin.left),
        finite_or_zero(margin.top),
        finite_or_zero(margin.right),
        finite_or_zero(margin.bottom),
    )
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
    // 有限负 margin/gap 保持既有语义，无界哨兵与非有限值回退为零。
    if value.is_finite() && value.abs() < f32::MAX {
        value
    } else {
        0.0
    }
}

fn flex_factor(value: f32) -> f32 {
    finite_non_negative(value)
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

// 把正剩余空间反复分给尚未触及主轴上限的子项。
fn distribute_positive_space<F>(
    base: &mut [f32],
    remaining: f32,
    children: &[FlexChild],
    is_row: bool,
    weight_for: F,
) where
    // 调用方决定使用 flex-grow 权重还是 Stretch 的均匀权重。
    F: Fn(&FlexChild) -> f32,
{
    // 使用 f64 账本避免多个极大有限权重在求和时溢出。
    let mut remaining = finite_non_negative(remaining) as f64;
    // 每轮至少冻结一个触顶项，因此最多需要子项数加一轮。
    for _round in 0..=base.len() {
        // 浮点尾差小于阈值时视为已经分配完成。
        if remaining <= 0.000_1 {
            // 结束正空间分配。
            break;
        }
        // 汇总仍可增长子项的有效权重。
        let total_weight: f64 = base
            .iter()
            .zip(children)
            .filter_map(|(&main, child)| {
                // 读取子项当前主轴最大值。
                let (_, maximum) = child_main_bounds(child, is_row);
                // 归一调用方提供的增长权重。
                let weight = finite_non_negative(weight_for(child));
                // 只保留有权重且尚未触顶的子项。
                (weight > 0.0 && main + 0.000_1 < maximum).then_some(weight as f64)
            })
            .sum();
        // 没有可增长子项时保留剩余空间给 justify 处理。
        if total_weight <= 0.0 {
            // 结束正空间分配。
            break;
        }
        // 记录本轮真正写入子项尺寸的空间。
        let mut consumed = 0.0f64;
        // 按权重尝试增长每个尚未触顶的子项。
        for (main, child) in base.iter_mut().zip(children) {
            // 读取当前子项主轴上限。
            let (_, maximum) = child_main_bounds(child, is_row);
            // 归一当前子项权重。
            let weight = finite_non_negative(weight_for(child));
            // 跳过无权重或已经触顶的子项。
            if weight <= 0.0 || *main + 0.000_1 >= maximum {
                // 继续处理下一个子项。
                continue;
            }
            // 按本轮总权重计算该子项应得份额。
            let share = remaining * weight as f64 / total_weight;
            // 把份额钳制到当前子项剩余增长容量。
            let growth = share.min((maximum - *main).max(0.0) as f64);
            // 将实际增长写回主轴尺寸。
            *main += growth as f32;
            // 累加本轮已消费空间。
            consumed += growth;
        }
        // 没有可观消费时避免在浮点尾差上空转。
        if consumed <= 0.000_1 {
            // 结束正空间分配。
            break;
        }
        // 未消费空间进入下一轮并排除已经触顶的子项。
        remaining = (remaining - consumed).max(0.0);
    }
}

// 把溢出反复分给尚未触及主轴下限的子项。
fn distribute_shrink(base: &mut [f32], overflow: f32, children: &[FlexChild], is_row: bool) {
    // 使用 f64 账本避免极大有限尺寸与权重相乘后溢出。
    let mut overflow = finite_non_negative(overflow) as f64;
    // 每轮至少冻结一个触底项，因此最多需要子项数加一轮。
    for _round in 0..=base.len() {
        // 浮点尾差小于阈值时视为已经吸收完溢出。
        if overflow <= 0.000_1 {
            // 结束收缩分配。
            break;
        }
        // 汇总仍可收缩子项的缩放权重。
        let total_shrink_weight: f64 = base
            .iter()
            .zip(children)
            .filter_map(|(&main, child)| {
                // 读取当前子项主轴下限。
                let (minimum, _) = child_main_bounds(child, is_row);
                // 归一 shrink 因子。
                let factor = flex_factor(child.flex_shrink);
                // 只保留有权重且仍高于下限的子项。
                (factor > 0.0 && main > minimum + 0.000_1).then_some(factor as f64 * main as f64)
            })
            .sum();
        // 没有可收缩子项时允许内容按最小尺寸溢出。
        if total_shrink_weight <= 0.0 {
            // 结束收缩分配。
            break;
        }
        // 记录本轮真正吸收的溢出。
        let mut consumed = 0.0f64;
        // 按缩放权重尝试收缩每个尚未触底的子项。
        for (main, child) in base.iter_mut().zip(children) {
            // 读取当前子项主轴下限。
            let (minimum, _) = child_main_bounds(child, is_row);
            // 归一 shrink 因子。
            let factor = flex_factor(child.flex_shrink);
            // 跳过无权重或已经触底的子项。
            if factor <= 0.0 || *main <= minimum + 0.000_1 {
                // 继续处理下一个子项。
                continue;
            }
            // 使用当前尺寸保持原有 scaled shrink 权重语义。
            let weight = factor as f64 * *main as f64;
            // 按本轮总权重计算应吸收的溢出。
            let share = overflow * weight / total_shrink_weight;
            // 把收缩量钳制到当前子项剩余容量。
            let reduction = share.min((*main - minimum).max(0.0) as f64);
            // 将实际收缩写回主轴尺寸。
            *main -= reduction as f32;
            // 累加本轮已吸收溢出。
            consumed += reduction;
        }
        // 没有可观消费时避免在浮点尾差上空转。
        if consumed <= 0.000_1 {
            // 结束收缩分配。
            break;
        }
        // 未吸收的溢出进入下一轮并排除已经触底的子项。
        overflow = (overflow - consumed).max(0.0);
    }
}

/// Single-line (no wrap) flex layout.
#[allow(
    clippy::too_many_arguments,
    reason = "the slices and axis flags are state for one flex layout pass"
)]
fn compute_single_line(
    input: &FlexInput<'_>,
    inner: &Rect,
    is_row: bool,
    is_reverse: bool,
    container_main: f32,
    container_cross: f32,
    intrinsic_main: bool,
    base_main_sizes: &mut [f32],
    cross_sizes: &mut [f32],
    child_rects: &mut Vec<Rect>,
) -> Size {
    let count = base_main_sizes.len();
    let gap = finite_or_zero(input.gap);
    // intrinsic_main（#165：未设主轴尺寸，由子项撑开）必须跳过 shrink：
    // 父级常先按 measure=0 分到过小 frame；若此时 shrink，Label 等会被压成 h=0，
    // 文字仍绘制 → 标题/描述重叠（首页快捷导航复现）。
    // bootstrap（container_main≈0）同样跳过，以便零高 frame 首次撑开。
    // 有明确主轴尺寸（style 设宽/高）时仍 shrink，以适配窗口/固定卡片。
    let bootstrap_main = container_main <= 1.0;
    let skip_shrink = intrinsic_main || bootstrap_main;
    // 固有交叉轴占位必须包含每个子项的两侧 margin。
    let max_child_cross = (0..count)
        .map(|index| cross_sizes[index] + margin_cross(child_margin(input, index), is_row))
        .fold(0.0, f32::max);
    // 自动交叉轴把子项自然外尺寸作为下限，同时保留父级已经分配的更大空间。
    let effective_cross = if input.intrinsic_cross || container_cross <= 0.0 {
        // 父级分配与自然内容取大，避免自动轴既裁内容又反向收缩拉伸区域。
        container_cross.max(max_child_cross)
    } else {
        // 确定交叉轴继续占满父级分配空间。
        container_cross
    };
    // Phase 2: distribute flex-grow/shrink
    let total_margin_main: f32 = (0..count)
        .map(|i| margin_main(child_margin(input, i), is_row))
        .sum();
    let total_base: f32 = base_main_sizes.iter().sum::<f32>() + total_margin_main;
    let gaps = gap * (count as f32 - 1.0);
    let overflow = if skip_shrink {
        0.0
    } else {
        (total_base + gaps - container_main).max(0.0)
    };

    if overflow > 0.0 {
        // 把溢出持续转交给尚未触及 min_size 的子项。
        distribute_shrink(base_main_sizes, overflow, input.children, is_row);
    }
    let total_after: f32 = base_main_sizes.iter().sum::<f32>() + total_margin_main;
    let mut remaining = if container_main > 0.0 {
        (container_main - total_after - gaps).max(0.0)
    } else {
        0.0
    };
    // grow 子项触及 max_size 后继续把剩余空间交给未触顶兄弟。
    distribute_positive_space(
        base_main_sizes,
        remaining,
        input.children,
        is_row,
        // 标准增长使用每个子项声明的 flex-grow 权重。
        |child| flex_factor(child.flex_grow),
    );

    // Apply Stretch justify-content: distribute remaining space as growth
    let total_after: f32 = base_main_sizes.iter().sum::<f32>() + total_margin_main;
    remaining = if container_main > 0.0 {
        (container_main - total_after - gaps).max(0.0)
    } else {
        0.0
    };
    if input.justify_content == JustifyContent::Stretch && remaining > 0.0 {
        // Stretch 使用均匀权重，并同样尊重每个子项的 max_size。
        distribute_positive_space(
            base_main_sizes,
            remaining,
            input.children,
            is_row,
            // 每个尚未触顶的子项获得相同权重。
            |_child| 1.0,
        );
    }

    // Phase 3: justify-content positioning
    // Reuse the most recent total to avoid an extra sum() traversal
    let total_final = base_main_sizes.iter().sum::<f32>() + total_margin_main;
    // bootstrap 保持自然起点；实际主轴保留负剩余空间供 Center/End 对齐未压缩的固有内容。
    remaining = if bootstrap_main {
        // 首次测量沿用自然内容起点，避免无约束对齐生成负坐标。
        0.0
    } else {
        // 实际容器内保留有限差值，让分布器处理固有内容溢出。
        finite_or_zero(container_main - total_final - gaps)
    };
    let (effective_gap, start_offset) =
        compute_justify(remaining, count, gap, input.justify_content);

    // Phase 4: position children
    child_rects.clear();
    child_rects.reserve(count);
    let mut cursor = start_offset;

    for i in 0..count {
        let margin = child_margin(input, i);
        let cross_align = input.children[i].align_self.unwrap_or(input.align_items);
        // 交叉轴先扣除两侧 margin，再在剩余区域内执行对齐。
        let available_cross = (effective_cross - margin_cross(margin, is_row)).max(0.0);
        // Stretch：父级交叉轴已确定（>1）时以可用区为候选，窗口缩小时允许小于 measure；
        // bootstrap（交叉轴仍 ≤1）时至少取 measured，最终候选仍须遵守交叉轴 min/max。
        let child_cross_size = if cross_align == AlignItems::Stretch {
            // Stretch 填满已经扣除 margin 的交叉轴可用区域。
            let filled = if container_cross <= 1.0 {
                available_cross.max(cross_sizes[i])
            } else {
                available_cross
            };
            // 读取当前子项交叉轴的有序上下限。
            let (minimum, maximum) = child_cross_bounds(&input.children[i], is_row);
            // 最终 Stretch 尺寸不得压破最小值或拉破最大值。
            filled.max(minimum).min(maximum)
        } else {
            cross_sizes[i]
        };

        let cross_offset = match cross_align {
            AlignItems::Start => 0.0,
            AlignItems::Center => (available_cross - child_cross_size) / 2.0,
            AlignItems::End => available_cross - child_cross_size,
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
        cursor += occupied_main + effective_gap;
    }

    // Reverse: 以主轴终点镜像子项位置
    if is_reverse {
        // 固有主轴只在零尺寸 bootstrap 使用内容长度，实际 frame 继续作为镜像边界。
        let main_extent = if intrinsic_main && bootstrap_main {
            // bootstrap 的固有主轴包含子项、margin 与 gap。
            total_final + gaps
        } else if is_row {
            // 水平固定主轴使用内容区宽度。
            inner.w
        } else {
            // 垂直固定主轴使用内容区高度。
            inner.h
        };
        for rect in child_rects.iter_mut() {
            if is_row {
                rect.x = inner.x + main_extent - (rect.x - inner.x) - rect.w;
            } else {
                rect.y = inner.y + main_extent - (rect.y - inner.y) - rect.h;
            }
        }
    }

    // total_size：intrinsic_main 始终用子项撑开（写入 cached_content_size，供下次 measure）；
    // 否则占满父级分配的主轴空间。
    let main_total = if intrinsic_main {
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

    Size::new(total_w, total_h)
}

/// Multi-line (wrapping) flex layout.
#[allow(
    clippy::too_many_arguments,
    reason = "the slices and axis flags are state for one wrapped flex layout pass"
)]
fn compute_wrapped(
    input: &FlexInput<'_>,
    inner: &Rect,
    is_row: bool,
    is_reverse: bool,
    container_main: f32,
    container_cross: f32,
    base_main_sizes: &mut [f32],
    cross_sizes: &mut [f32],
    lines: &mut Vec<FlexLine>,
    line_cross_positions: &mut Vec<f32>,
    line_max_cross: &mut Vec<f32>,
    child_rects: &mut Vec<Rect>,
) -> Size {
    let count = base_main_sizes.len();
    let gap = finite_or_zero(input.gap);
    // 零或近零主轴属于首次 bootstrap，不应被当成真实换行上限。
    let bootstrap_main = container_main <= 1.0;
    // 固有主轴或 bootstrap 阶段保持子项自然尺寸，不执行 shrink。
    let skip_shrink = input.intrinsic_main || bootstrap_main;
    // bootstrap 使用内部无界约束聚合自然尺寸，实际输出仍会保持有限。
    let wrap_limit = if bootstrap_main {
        // 无真实主轴约束时把所有子项留在自然行中。
        f32::MAX
    } else {
        // 已有真实主轴尺寸时按容器边界换行。
        container_main
    };

    // 在树级工作区内重建行范围，避免每个换行容器重复申请。
    lines.clear();
    let mut line_start = 0usize;
    let mut line_main = 0.0f32;

    for (i, &base_main_size) in base_main_sizes.iter().enumerate() {
        let child_main = base_main_size + margin_main(child_margin(input, i), is_row);
        let item_gap = if i > line_start { gap } else { 0.0 };

        // 当前行只要已经收集过子项，就必须让超出上限的新项另起一行。
        if line_main + item_gap + child_main > wrap_limit && i > line_start {
            lines.push(FlexLine {
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
        lines.push(FlexLine {
            start: line_start,
            end: count,
        });
    }

    if lines.is_empty() {
        child_rects.clear();
        return Size::new(inner.w, inner.h);
    }

    // Track cross-axis position for each line
    let line_gap = gap;
    line_cross_positions.clear();
    line_cross_positions.reserve(lines.len());
    line_max_cross.clear();
    line_max_cross.reserve(lines.len());
    let mut cursor_cross = 0.0f32;

    for line in lines.iter() {
        let line_count = line.end - line.start;
        let line_margin_main: f32 = (line.start..line.end)
            .map(|i| margin_main(child_margin(input, i), is_row))
            .sum();
        let line_base: f32 =
            base_main_sizes[line.start..line.end].iter().sum::<f32>() + line_margin_main;
        let line_gaps = gap * (line_count as f32 - 1.0).max(0.0);
        // 固有或 bootstrap 主轴不得因临时容器尺寸压缩自然内容。
        let overflow = if skip_shrink {
            // 保持自然尺寸。
            0.0
        } else {
            // 有真实约束时计算当前行溢出。
            (line_base + line_gaps - container_main).max(0.0)
        };

        // Shrink within line
        if overflow > 0.0 {
            // 持续转交溢出直到所有可收缩项完成或触及 min_size。
            distribute_shrink(
                &mut base_main_sizes[line.start..line.end],
                overflow,
                &input.children[line.start..line.end],
                is_row,
            );
        }

        // Grow within line
        let remaining = (container_main
            - base_main_sizes[line.start..line.end].iter().sum::<f32>()
            - line_margin_main
            - line_gaps)
            .max(0.0);
        // grow 项触及 max_size 后继续把空间交给同一行的未触顶兄弟。
        distribute_positive_space(
            &mut base_main_sizes[line.start..line.end],
            remaining,
            &input.children[line.start..line.end],
            is_row,
            // 标准增长使用每个子项的 flex-grow 权重。
            |child| flex_factor(child.flex_grow),
        );

        // Apply Stretch
        let total_after: f32 =
            base_main_sizes[line.start..line.end].iter().sum::<f32>() + line_margin_main;
        let remaining2 = (container_main - total_after - line_gaps).max(0.0);
        if input.justify_content == JustifyContent::Stretch && remaining2 > 0.0 {
            // Stretch 均匀分配空间并尊重每个子项的 max_size。
            distribute_positive_space(
                &mut base_main_sizes[line.start..line.end],
                remaining2,
                &input.children[line.start..line.end],
                is_row,
                // 同一行每个尚未触顶的子项使用相同权重。
                |_child| 1.0,
            );
        }

        // Compute cross size for this line
        let max_cross = (line.start..line.end)
            .map(|i| cross_sizes[i] + margin_cross(child_margin(input, i), is_row))
            .fold(0.0, f32::max);
        line_cross_positions.push(cursor_cross);
        line_max_cross.push(max_cross);
        cursor_cross += max_cross + line_gap;
    }

    // Position children line by line
    child_rects.clear();
    child_rects.resize(count, Rect::zero());

    // 自然交叉轴总尺寸必须覆盖所有行的最远物理末端。
    let natural_total_cross = line_cross_positions
        // 把每行起点与对应行高配对。
        .iter()
        // 负 gap 可能让较矮末行早于前面高行结束，因此不能只读取最终游标。
        .zip(line_max_cross.iter())
        // 逐行计算相对内容原点的有限物理末端。
        .map(|(&start, &size)| finite_or_zero(start + size))
        // 取所有行末端最大值，并把完全位于原点前的范围收敛为零。
        .fold(0.0, f32::max);
    // 自动交叉轴同样保留父级分配值，后续行账本再以自然尺寸作为下限。
    let layout_cross = container_cross;
    // 容器级 Stretch 同时承担多行交叉轴分布，让 wrap 开关不改变单行填充语义。
    let cross_align = input.align_items;
    // 实际未换行时应与非换行路径共享完整容器交叉轴行盒。
    let single_line_uses_container_cross = lines.len() == 1 && layout_cross > 1.0;
    // 单行直接采用真实行盒，让逐项 align_self 也能相对容器定位。
    if single_line_uses_container_cross {
        // 唯一行覆盖为容器交叉轴，子项最终尺寸仍会服从各自 min/max。
        line_max_cross[0] = layout_cross;
    // 只有实际容器还存在正交叉轴剩余空间时才扩展行盒。
    } else if cross_align == AlignItems::Stretch && layout_cross > natural_total_cross {
        // 扣除自然行高与固定 gap 后，把剩余空间等分到每一行。
        let extra_per_line = (layout_cross - natural_total_cross) / lines.len() as f32;
        // 原位同步每行起点与行高，不复制子项或新增第二套行账本。
        for (line_index, line_cross_size) in line_max_cross.iter_mut().enumerate() {
            // 前置行获得的扩展量会共同推后当前行起点。
            line_cross_positions[line_index] += extra_per_line * line_index as f32;
            // 当前行自身消费一份交叉轴剩余空间。
            *line_cross_size += extra_per_line;
        }
    }
    // Stretch 与真实单行都覆盖容器交叉尺寸，报告账本仍保留自然溢出。
    let total_cross = if cross_align == AlignItems::Stretch || single_line_uses_container_cross {
        // 容器较小或 bootstrap 时不得压低自然行组尺寸。
        natural_total_cross.max(layout_cross)
    } else {
        // Center/End/Start 保留既有自然行组范围。
        natural_total_cross
    };
    // 单行已经在真实行盒内逐项对齐，不再额外移动整组。
    let cross_start_offset = if single_line_uses_container_cross {
        // 唯一行从容器交叉轴起点开始。
        0.0
    // 多行继续沿用既有的自然行组对齐语义。
    } else {
        // 根据容器级对齐计算整组行盒偏移。
        match cross_align {
            // 只有获得实际交叉轴后才应用 Center，bootstrap 继续从自然起点开始。
            AlignItems::Center if layout_cross > 1.0 => (layout_cross - total_cross) * 0.5,
            // 获得实际交叉轴后 End 保留负剩余空间，使自然行组末端贴住容器末端。
            AlignItems::End if layout_cross > 1.0 => layout_cross - total_cross,
            // Start 与 Stretch 从自然交叉轴起点开始。
            _ => 0.0,
        }
    };
    // 记录所有行中真实占用的最大主轴长度，供固有尺寸与反向布局使用。
    let mut max_line_main = 0.0f32;

    for (li, line) in lines.iter().enumerate() {
        let line_count = line.end - line.start;
        let line_gaps_total = gap * (line_count as f32 - 1.0).max(0.0);
        let line_margin_main: f32 = (line.start..line.end)
            .map(|i| margin_main(child_margin(input, i), is_row))
            .sum();
        let total_line_main: f32 =
            base_main_sizes[line.start..line.end].iter().sum::<f32>() + line_margin_main;
        // 更新所有行包含声明 gap 的最大主轴占用。
        max_line_main = max_line_main.max((total_line_main + line_gaps_total).max(0.0));
        // bootstrap 保持自然行起点；实际主轴保留负剩余空间供 Center/End 对齐超宽行。
        let remaining = if bootstrap_main {
            // 首次测量沿用自然行起点，避免无约束对齐生成负坐标。
            0.0
        } else {
            // 实际容器内保留有限差值，让分布器处理不可拆分的超宽行。
            finite_or_zero(container_main - total_line_main - line_gaps_total)
        };
        let (effective_gap, start_offset) =
            compute_justify(remaining, line_count, gap, input.justify_content);

        let mut cursor_main = start_offset;
        // 可见末端账本独立于 justify 偏移，并按声明 gap 自然推进。
        let mut natural_cursor_main = 0.0f32;
        let line_cross_base = line_cross_positions[li] + cross_start_offset;
        for i in line.start..line.end {
            let margin = child_margin(input, i);
            let cross_align = input.children[i].align_self.unwrap_or(input.align_items);
            // 每个子项先扣除当前行交叉轴两侧 margin 再执行对齐。
            let available_cross = (line_max_cross[li] - margin_cross(margin, is_row)).max(0.0);
            let child_cross_size = if cross_align == AlignItems::Stretch {
                // 读取当前子项交叉轴的有序上下限。
                let (minimum, maximum) = child_cross_bounds(&input.children[i], is_row);
                // Stretch 以行盒可用区为候选，但最终仍须遵守子项约束。
                available_cross.max(minimum).min(maximum)
            } else {
                cross_sizes[i]
            };

            let cross_offset = match cross_align {
                AlignItems::Start => 0.0,
                AlignItems::Center => (available_cross - child_cross_size) / 2.0,
                AlignItems::End => available_cross - child_cross_size,
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
            // 负尾侧 margin 不得裁掉当前子项的自然可见末端。
            let visible_main_end = natural_cursor_main
                + margin_main_start(margin, is_row, is_reverse)
                + base_main_sizes[i];
            // 全部行共同保留最远的有限主轴可见末端。
            max_line_main = max_line_main.max(finite_or_zero(visible_main_end));
            // 下一项仍按包含 margin 与声明 gap 的自然占用推进。
            natural_cursor_main += occupied_main + gap;
            cursor_main += occupied_main + effective_gap;
        }
    }

    // Reverse: 以主轴终点镜像子项位置
    if is_reverse {
        // 固有主轴只在零尺寸 bootstrap 使用最大行长，实际 frame 继续作为镜像边界。
        let main_extent = if input.intrinsic_main && bootstrap_main {
            // bootstrap 的固有尺寸取所有行的最大主轴占用。
            max_line_main
        } else if is_row {
            // 水平固定主轴使用内容区宽度。
            inner.w
        } else {
            // 垂直固定主轴使用内容区高度。
            inner.h
        };
        for rect in child_rects.iter_mut() {
            if is_row {
                rect.x = inner.x + main_extent - (rect.x - inner.x) - rect.w;
            } else {
                rect.y = inner.y + main_extent - (rect.y - inner.y) - rect.h;
            }
        }
    }

    // 固有主轴由最大行长撑开，固定主轴继续占满父级分配空间。
    let resolved_main = if input.intrinsic_main {
        max_line_main
    } else {
        container_main.max(0.0)
    };
    // 可见交叉轴总量还须覆盖负尾侧 margin 未裁剪的子项末端。
    let visible_cross_end = child_rects.iter().fold(0.0f32, |extent, rect| {
        let end = if is_row {
            rect.y + rect.h - inner.y
        } else {
            rect.x + rect.w - inner.x
        };
        extent.max(finite_or_zero(end))
    });
    let (total_w, total_h) = if is_row {
        // 水平布局的交叉轴总量统一使用行账本，单行也包含 margin。
        let resolved_cross = total_cross.max(layout_cross).max(visible_cross_end);
        (
            resolved_main + input.padding.horizontal(),
            resolved_cross + input.padding.vertical(),
        )
    } else {
        // 垂直布局的交叉轴总量采用相同行账本语义。
        let resolved_cross = total_cross.max(layout_cross).max(visible_cross_end);
        (
            resolved_cross + input.padding.horizontal(),
            resolved_main + input.padding.vertical(),
        )
    };
    Size::new(total_w, total_h)
}

// 仅在库测试中加载独立的 min/max 弹性分配契约。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/ui/layout/flex/tests.rs"]
mod tests;
