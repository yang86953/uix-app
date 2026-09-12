// 拆分自 grid.rs：Auto/Fr 轨道尺寸求解与有限化辅助。
// 引入轨道、放置账本与子项类型。
use super::{CellAssignment, EdgeInsets, GridChild, GridTrack, GridTrackMax, GridTrackMin};

// 混合 Auto/Fr 跨轨约束只做固定轮数的单调松弛，避免病理输入形成无界循环。
const FRACTION_SPAN_RELAXATION_LIMIT: usize = 64;

pub(super) fn finite_non_negative(value: f32) -> f32 {
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

pub(super) fn finite_insets(insets: EdgeInsets) -> EdgeInsets {
    EdgeInsets::new(
        finite_or_zero(insets.left),
        finite_or_zero(insets.top),
        finite_or_zero(insets.right),
        finite_or_zero(insets.bottom),
    )
}

// 在进入求解前拒绝绕过 GridTrack::minmax 构造器的非法分量，与构造器同一契约。
pub(super) fn expect_legal_tracks(tracks: &[GridTrack]) {
    for track in tracks {
        if let GridTrack::MinMax(min, max) = track {
            let (GridTrackMin::Px(lower) | GridTrackMin::Percent(lower)) = *min;
            let (GridTrackMax::Px(upper) | GridTrackMax::Fr(upper)) = *max;
            assert!(
                lower.is_finite() && lower >= 0.0 && upper.is_finite() && upper >= 0.0,
                "GridTrack::MinMax requires finite, non-negative bounds"
            );
        }
    }
}

// 把 minmax 下界按容器同轴内容盒解析为像素；百分比参照未定时为 0。
fn minmax_floor(min: GridTrackMin, available: Option<f32>) -> f32 {
    match min {
        GridTrackMin::Px(px) => finite_non_negative(px),
        GridTrackMin::Percent(percent) => available
            .filter(|value| value.is_finite() && *value > 0.0)
            .map(|value| finite_non_negative(value * finite_non_negative(percent) / 100.0))
            .unwrap_or(0.0),
    }
}

// 轨道在剩余空间分配中的 fr 权重：Fr 与 fr 上界的 minmax 参与，其他为 0。
fn fraction_weight(track: &GridTrack) -> f64 {
    match track {
        GridTrack::Fr(weight) | GridTrack::MinMax(_, GridTrackMax::Fr(weight)) => {
            finite_non_negative(*weight) as f64
        }
        GridTrack::Px(_) | GridTrack::Auto | GridTrack::MinMax(_, GridTrackMax::Px(_)) => 0.0,
    }
}

// 轨道在固有尺寸账本中保证占用的像素：Px 用自身，minmax 用下界，Auto/Fr 由调用方处理。
fn guaranteed_extent(track: &GridTrack, available: Option<f32>) -> f64 {
    match track {
        GridTrack::Px(px) => finite_non_negative(*px) as f64,
        GridTrack::MinMax(min, _) => minmax_floor(*min, available) as f64,
        GridTrack::Auto | GridTrack::Fr(_) => 0.0,
    }
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
pub(super) fn intrinsic_auto_track_sizes_into(
    tracks: &[GridTrack],
    assignments: &[CellAssignment],
    children: &[GridChild],
    gap: f32,
    horizontal: bool,
    sizes: &mut Vec<f32>,
    spanning_indices: &mut Vec<usize>,
    planned_increases: &mut Vec<f32>,
) {
    // 为每条轨道建立独立的固有尺寸账本。
    sizes.clear();
    sizes.resize(tracks.len(), 0.0);

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

    // 只收集真正跨越多条轨道的子项，避免单轨道贡献重复结算。
    spanning_indices.clear();
    spanning_indices.extend(
        assignments
            // 借用既有定位账本，不复制子项或轨道数据。
            .iter()
            .enumerate()
            // 按当前轴过滤掉空 span 与单轨道 span。
            .filter(|(_, assignment)| {
                // 水平轴读取列 span，垂直轴读取行 span。
                let span = if horizontal {
                    // 列 span 已由放置阶段收敛到轨道预算。
                    assignment.col_span as usize
                } else {
                    // 行 span 已由放置阶段收敛到轨道预算。
                    assignment.row_span as usize
                };
                // 只有至少覆盖两条轨道的子项进入后续批次。
                span > 1
            })
            // 物化有界引用表，供跨度排序和同批结算复用。
            .map(|(index, _)| index),
    );
    // 较短 span 先建立基础尺寸，同跨度子项保持同批处理。
    spanning_indices.sort_by_key(|&index| {
        let assignment = &assignments[index];
        // 排序键只依赖布局约束，不依赖子项声明顺序。
        if horizontal {
            // 水平轴按有界列 span 升序排列。
            assignment.col_span
        } else {
            // 垂直轴按有界行 span 升序排列。
            assignment.row_span
        }
    });
    // 每条轨道只记录当前跨度批次要求的最大计划增量。
    planned_increases.clear();
    planned_increases.resize(tracks.len(), 0.0);
    // 从排序后的首个跨格子项开始扫描。
    let mut group_start = 0usize;
    // 每轮处理一组跨度相同的子项。
    while group_start < spanning_indices.len() {
        // 读取当前批次在目标轴上的统一 span。
        let group_span = if horizontal {
            // 水平轴使用当前首项的列 span。
            assignments[spanning_indices[group_start]].col_span
        } else {
            // 垂直轴使用当前首项的行 span。
            assignments[spanning_indices[group_start]].row_span
        };
        // 至少把当前首项纳入批次。
        let mut group_end = group_start + 1;
        // 向后收集所有跨度相同的连续子项。
        while group_end < spanning_indices.len() {
            // 读取候选子项在目标轴上的 span。
            let candidate_span = if horizontal {
                // 水平轴读取候选列 span。
                assignments[spanning_indices[group_end]].col_span
            } else {
                // 垂直轴读取候选行 span。
                assignments[spanning_indices[group_end]].row_span
            };
            // 遇到下一种跨度时结束当前批次。
            if candidate_span != group_span {
                // 保留下一批的起始索引。
                break;
            }
            // 将同跨度候选纳入当前批次。
            group_end += 1;
        }
        // 清除上一批留下的计划值，同时复用同一有界缓冲。
        planned_increases.fill(0.0);

        // 同跨度子项全部基于批次开始时的同一轨道快照计算贡献。
        for &assignment_index in &spanning_indices[group_start..group_end] {
            let assignment = &assignments[assignment_index];
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
            // 含正比例 Fr（含 fr 上界 minmax）的 span 由后续剩余空间分配承担。
            let contains_fraction = tracks[start..end]
                .iter()
                .any(|track| fraction_weight(track) > 0.0);
            // 保留 Fr 轨道吸收剩余空间的既有语义。
            if contains_fraction {
                // 这类 span 不额外扩张 Auto 轨道。
                continue;
            }
            // 统计 span 中可承担缺口的 Auto 轨道数。
            let auto_count = tracks[start..end]
                // 遍历当前子项真正覆盖的轨道。
                .iter()
                // 固定与比例轨道不参与 Auto 缺口分摊。
                .filter(|track| matches!(track, GridTrack::Auto))
                // 得到可分摊轨道总数。
                .count();
            // 没有 Auto 轨道时不应改写固定轨道。
            if auto_count == 0 {
                // 当前 span 没有可分摊的轨道。
                continue;
            }
            // 用 f64 账本避免多条轨道求和时提前溢出。
            let track_extent = tracks[start..end]
                // 遍历 span 内的轨道定义。
                .iter()
                // 保留相对起点以读取对应 Auto 尺寸。
                .enumerate()
                // 把不同轨道类型转换为当前已占用尺寸。
                .map(|(offset, track)| {
                    // 固定轨道计入自身宽高，Auto 计入批次开始时的尺寸。
                    match track {
                        // 当前 Auto 轨道使用上一批已确定的尺寸。
                        GridTrack::Auto => sizes[start + offset] as f64,
                        // 固定轨道用像素值，minmax 用下界（此阶段无容器参照，百分比为 0），
                        // 非正 Fr 在这一固有尺寸账本中不占空间。
                        other => guaranteed_extent(other, None),
                    }
                })
                // 汇总整个 span 已有的轨道尺寸。
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
            let share = finite_non_negative((deficit / auto_count as f64) as f32);
            // 为当前子项覆盖的每条 Auto 轨道登记计划增量。
            for index in start..end {
                // 固定和比例轨道不承担 Auto 尺寸缺口。
                if matches!(tracks[index], GridTrack::Auto) {
                    // 同批重叠约束取最大贡献，避免声明顺序重复累加。
                    planned_increases[index] = planned_increases[index].max(share);
                }
            }
        }

        // 当前跨度的所有约束完成后再统一写回轨道尺寸。
        for (index, increase) in planned_increases.iter().copied().enumerate() {
            // 只有本批实际要求扩张的轨道需要写回。
            if increase > 0.0 {
                // 有界加法继续通过尺寸边界收敛。
                sizes[index] = finite_non_negative((sizes[index] as f64 + increase as f64) as f32);
            }
        }
        // 下一轮从后续跨度批次开始。
        group_start = group_end;
    }
}

// 在不扩大父级约束的前提下满足 Auto/Fr 混合 span 的自然尺寸。
pub(super) fn fit_fraction_spanning_auto_tracks(
    // 当前轴的全部有界轨道。
    tracks: &[GridTrack],
    // 已成功放置的子项与 span 账本。
    assignments: &[CellAssignment],
    // 子项自然尺寸与 margin 来源。
    children: &[GridChild],
    // 当前轴相邻轨道间距。
    gap: f32,
    // 当前轴父级可用尺寸。
    available: f32,
    // 当前轴全部轨道间距之和。
    total_gap: f32,
    // true 表示列轴，false 表示行轴。
    horizontal: bool,
    // 前一阶段建立的 Auto 固有尺寸账本。
    auto_sizes: &mut [f32],
    // 调用方复用的跨轨子项索引。
    spanning_indices: &mut Vec<usize>,
) {
    // 汇总当前轴所有有效 Fr 权重，供 span 内外份额换算。
    let total_fraction_weight = tracks
        // 遍历有界轨道定义。
        .iter()
        // Fr 与 fr 上界的 minmax 贡献权重。
        .map(fraction_weight)
        // 使用 f64 避免极大权重求和溢出。
        .sum::<f64>();
    // 没有有效 Fr 时，前一阶段已经完成全部 Auto 跨轨贡献。
    if total_fraction_weight <= 0.0 {
        // 直接保留既有 Auto 账本。
        return;
    }

    // 收集真正跨越多轨道的约束，后续按约束本身确定顺序。
    spanning_indices.clear();
    spanning_indices.extend(
        assignments
            // 借用既有有界放置账本。
            .iter()
            .enumerate()
            // 单轨道子项已经在 Auto 基础阶段处理。
            .filter(|(_, assignment)| {
                // 水平轴读取列 span，垂直轴读取行 span。
                let span = if horizontal {
                    // 列 span 已由放置预算收敛。
                    assignment.col_span
                } else {
                    // 行 span 已由放置预算收敛。
                    assignment.row_span
                };
                // 只有真正跨轨的子项需要混合约束松弛。
                span > 1
            })
            // 物化有界引用表以消除声明顺序影响。
            .map(|(index, _)| index),
    );
    // 按 span、起点与自然外尺寸建立确定性处理顺序。
    spanning_indices.sort_by(|&left_index, &right_index| {
        let left = &assignments[left_index];
        let right = &assignments[right_index];
        // 读取左侧约束的起始轨道。
        let left_start = if horizontal { left.col } else { left.row };
        // 读取右侧约束的起始轨道。
        let right_start = if horizontal { right.col } else { right.row };
        // 读取左侧约束的轨道跨度。
        let left_span = if horizontal {
            left.col_span
        } else {
            left.row_span
        };
        // 读取右侧约束的轨道跨度。
        let right_span = if horizontal {
            right.col_span
        } else {
            right.row_span
        };
        // 较短 span 先建立局部约束，再按起点稳定排序。
        left_span
            // 首要按跨度升序。
            .cmp(&right_span)
            // 同跨度按轨道起点升序。
            .then_with(|| left_start.cmp(&right_start))
            // 同范围优先处理较大的自然尺寸，减少重复松弛。
            .then_with(|| {
                // 读取右侧子项自然外尺寸。
                let right_extent = child_outer_extent(&children[right.child_idx], horizontal);
                // 读取左侧子项自然外尺寸。
                let left_extent = child_outer_extent(&children[left.child_idx], horizontal);
                // 使用浮点全序按尺寸降序排列。
                right_extent.total_cmp(&left_extent)
            })
    });
    // 没有跨轨约束时无需进入松弛循环。
    if spanning_indices.is_empty() {
        // 保留已有 Auto 尺寸。
        return;
    }

    // 计算固定 Px 与当前 Auto 已占用的全局空间。
    let mut used_non_fraction = tracks
        // 保留轨道索引以读取对应 Auto 尺寸。
        .iter()
        // 把每条非 Fr 轨道转换为已占用尺寸。
        .enumerate()
        // 汇总固定与内容轨道占用。
        .map(|(index, track)| match track {
            // Auto 轨道使用前一阶段的固有尺寸。
            GridTrack::Auto => auto_sizes.get(index).copied().unwrap_or(0.0) as f64,
            // 固定轨道与 minmax 下界是保证占用；纯 Fr 留到剩余空间阶段分配。
            other => guaranteed_extent(other, Some(available)),
        })
        // 使用 f64 累加避免多轨道求和溢出。
        .sum::<f64>();
    // 父级可分配给轨道的空间先扣除全部 gap。
    let track_capacity =
        (finite_non_negative(available) as f64 - finite_non_negative(total_gap) as f64).max(0.0);

    // 多个重叠混合 span 可能互相改变 Fr 余量，因此执行固定上限的单调松弛。
    for _ in 0..FRACTION_SPAN_RELAXATION_LIMIT {
        // 本轮尚未增加任何 Auto 尺寸。
        let mut made_progress = false;
        // 按确定性约束顺序逐项消除可由外部 Fr 让出的缺口。
        for &assignment_index in spanning_indices.iter() {
            let assignment = &assignments[assignment_index];
            // 按当前轴读取起始轨道。
            let start = if horizontal {
                assignment.col
            } else {
                assignment.row
            };
            // 按当前轴读取有界 span。
            let span = if horizontal {
                // 水平轴使用列 span。
                assignment.col_span as usize
            } else {
                // 垂直轴使用行 span。
                assignment.row_span as usize
            };
            // 将结束位置限制在实际轨道窗口内。
            let end = start.saturating_add(span).min(tracks.len());
            // 统计 span 内可承接转移份额的 Auto 轨道数量。
            let auto_count = tracks[start..end]
                // 遍历当前 span 的轨道定义。
                .iter()
                // 只有 Auto 能在本阶段增长。
                .filter(|track| matches!(track, GridTrack::Auto))
                // 得到可均分转移量的轨道数。
                .count();
            // 没有 Auto 时不能在不改写 Fr 定义的前提下调整。
            if auto_count == 0 {
                // 保留既有 Fr 分配语义。
                continue;
            }
            // 汇总 span 内的有效 Fr 权重。
            let span_fraction_weight = tracks[start..end]
                // 遍历当前 span 的轨道定义。
                .iter()
                // 提取 Fr 与 fr 上界 minmax 的有效权重。
                .map(fraction_weight)
                // 使用 f64 汇总权重。
                .sum::<f64>();
            // 不含正 Fr 的 span 已由前一阶段处理。
            if span_fraction_weight <= 0.0 {
                // 避免重复扩张纯 Auto/Px span。
                continue;
            }
            // 只有 span 外 Fr 的份额能在保持总宽高不变时转入 span。
            let external_fraction_share =
                (1.0 - span_fraction_weight / total_fraction_weight).max(0.0);
            // span 覆盖全部 Fr 时，内部转移只会等量替换自身份额。
            if external_fraction_share <= f64::EPSILON {
                // 该约束需要独立的溢出策略，本批不改写父级约束。
                continue;
            }
            // 读取当前全局剩余空间。
            let remaining = (track_capacity - used_non_fraction).max(0.0);
            // Fr 已无剩余时不能继续在固定容器内转移。
            if remaining <= f64::EPSILON {
                // 保留当前已达到的最紧约束结果。
                continue;
            }
            // 计算当前 span 已获得的固定、Auto 与 Fr 尺寸。
            let span_track_extent = tracks[start..end]
                // 保留相对索引以读取 Auto 账本。
                .iter()
                // 将各类轨道转换为当前实际份额。
                .enumerate()
                // 逐轨道建立当前尺寸。
                .map(|(offset, track)| match track {
                    // Auto 轨道使用当前松弛后的尺寸。
                    GridTrack::Auto => auto_sizes[start + offset] as f64,
                    // 其他轨道取保证占用与按全局权重分享的剩余空间之大者。
                    other => {
                        let share = remaining * fraction_weight(other) / total_fraction_weight;
                        guaranteed_extent(other, Some(available)).max(share)
                    }
                })
                // 汇总当前 span 的轨道尺寸。
                .sum::<f64>();
            // span 内部 gap 同样属于子项可用外尺寸。
            let span_gap = gap as f64 * end.saturating_sub(start + 1) as f64;
            // 读取当前子项的自然外尺寸要求。
            let required = child_outer_extent(&children[assignment.child_idx], horizontal) as f64;
            // 计算尚未覆盖的自然尺寸缺口。
            let deficit = (required - span_track_extent - span_gap).max(0.0);
            // 已满足的约束不需要调整。
            if deficit <= 1.0e-6 {
                // 继续检查后续混合 span。
                continue;
            }
            // Auto 增量只有外部 Fr 占比部分会转化为当前 span 的净增长。
            let requested_transfer = deficit / external_fraction_share;
            // 转移量不能超过当前全部 Fr 剩余空间。
            let transfer = requested_transfer.min(remaining);
            // 极小转移不再改写 f32 账本。
            if transfer <= 1.0e-6 {
                // 防止浮点尾差导致空转。
                continue;
            }
            // 把总转移量均匀分摊给 span 内 Auto 轨道。
            let share = finite_non_negative((transfer / auto_count as f64) as f32);
            // 记录本次实际写入的总增量。
            let mut applied = 0.0f64;
            // 逐条扩张 span 内的 Auto 轨道。
            for index in start..end {
                // 固定与 Fr 轨道不接收转移量。
                if !matches!(tracks[index], GridTrack::Auto) {
                    // 跳过非 Auto 轨道。
                    continue;
                }
                // 保存写入前尺寸以计算实际 f32 增量。
                let previous = auto_sizes[index];
                // 使用有限化加法更新 Auto 尺寸。
                auto_sizes[index] = finite_non_negative(previous + share);
                // 累计实际写入量，保持后续 remaining 与 f32 账本一致。
                applied += (auto_sizes[index] - previous) as f64;
            }
            // 没有可表示的 f32 增量时停止推进该约束。
            if applied <= 0.0 {
                // 避免后续轮次重复空转。
                continue;
            }
            // 全局非 Fr 占用同步增加，后续约束读取最新 Fr 余量。
            used_non_fraction += applied;
            // 标记本轮发生了有效松弛。
            made_progress = true;
        }
        // 一整轮没有任何可表示增量时已经收敛。
        if !made_progress {
            // 提前退出固定上限循环。
            break;
        }
    }
}

// 在固定与 Auto 尺寸确定后，先让固定上界的 minmax 轨道增长，再把剩余空间分配给 fr 轨道。
pub(super) fn resolve_tracks_into(
    tracks: &[GridTrack],
    available: f32,
    total_gap: f32,
    auto_sizes: &[f32],
    sizes: &mut Vec<f32>,
) {
    // 为每条轨道建立最终尺寸账本。
    sizes.clear();
    sizes.resize(tracks.len(), 0.0);
    let available = finite_non_negative(available);
    // 百分比下界只参照已确定（正有限）的容器轴。
    let reference = (available > 0.0).then_some(available);
    // 使用 f64 累加已确定尺寸，避免多轨道求和溢出。
    let mut used = 0.0f64;
    // 使用 f64 累加比例权重，避免极大权重求和溢出。
    let mut total_fr = 0.0f64;

    // 阶段一：锁定 Px、Auto 与全部 minmax 下界，并汇总有效 fr 权重。
    for (index, track) in tracks.iter().enumerate() {
        match track {
            GridTrack::Px(px) => {
                let size = finite_non_negative(*px);
                sizes[index] = size;
                used += size as f64;
            }
            GridTrack::Auto => {
                let size = auto_sizes
                    .get(index)
                    .copied()
                    .map(finite_non_negative)
                    .unwrap_or(0.0);
                sizes[index] = size;
                used += size as f64;
            }
            GridTrack::MinMax(min, max) => {
                // 下界先于一切分配占用空间；fr 上界的下界同样是保证占用。
                let floor = minmax_floor(*min, reference);
                sizes[index] = floor;
                used += floor as f64;
                if let GridTrackMax::Fr(weight) = max {
                    total_fr += finite_non_negative(*weight) as f64;
                }
            }
            GridTrack::Fr(fr) => {
                total_fr += finite_non_negative(*fr) as f64;
            }
        }
    }

    // 可用空间先扣除 gap 与全部保证占用。
    let mut remaining = (available as f64 - finite_non_negative(total_gap) as f64 - used).max(0.0);

    // 阶段二：把剩余空间均匀分给尚未触顶的固定上界 minmax 轨道；上界低于下界时抬到下界。
    loop {
        let growable: Vec<usize> = tracks
            .iter()
            .enumerate()
            .filter_map(|(index, track)| match track {
                GridTrack::MinMax(_, GridTrackMax::Px(cap)) => {
                    let cap = finite_non_negative(*cap).max(sizes[index]);
                    (cap > sizes[index] + 1.0e-6).then_some(index)
                }
                _ => None,
            })
            .collect();
        if growable.is_empty() || remaining <= 1.0e-6 {
            break;
        }
        let share = remaining / growable.len() as f64;
        let mut applied = 0.0f64;
        for index in growable {
            let GridTrack::MinMax(_, GridTrackMax::Px(cap)) = tracks[index] else {
                continue;
            };
            let cap = finite_non_negative(cap).max(sizes[index]);
            let next = finite_non_negative(((sizes[index] as f64) + share).min(cap as f64) as f32);
            applied += (next - sizes[index]) as f64;
            sizes[index] = next;
        }
        remaining = (remaining - applied).max(0.0);
        if applied <= 1.0e-6 {
            break;
        }
    }

    // 没有可分配空间或有效 fr 权重时，弹性轨道停在各自下界（纯 Fr 为 0）。
    if remaining <= 0.0 || total_fr <= 0.0 {
        // 纯 Auto 网格因此不会无条件填满父容器。
        return;
    }

    // 阶段三：fr 分配。下界高于自身份额的 minmax 轨道按下界冻结并让出份额，
    // 直到剩余弹性轨道都能得到不低于下界的尺寸；有界轮数保证终止。
    let mut frozen = vec![false; tracks.len()];
    let mut pool_weight = total_fr;
    // 真正参与 fr 分配（权重为正）的轨道，其下界已计入 used，分配时放回可分配空间；
    // 0fr 等零权重轨道不参与分配，下界只保留占用，不得重复送入分配池。
    let flexible_floors: f64 = tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| fraction_weight(track) > 0.0)
        .map(|(index, _)| sizes[index] as f64)
        .sum();
    let mut distributable = remaining + flexible_floors;
    for _ in 0..=tracks.len() {
        if pool_weight <= 0.0 {
            break;
        }
        let unit = distributable / pool_weight;
        let mut changed = false;
        for (index, track) in tracks.iter().enumerate() {
            if frozen[index] {
                continue;
            }
            let weight = fraction_weight(track);
            if weight <= 0.0 {
                continue;
            }
            let floor = sizes[index] as f64;
            if floor > unit * weight + 1.0e-6 {
                // 份额不足下界：按下界冻结，并从可分配空间与权重池中移除。
                frozen[index] = true;
                distributable = (distributable - floor).max(0.0);
                pool_weight -= weight;
                changed = true;
            }
        }
        if !changed {
            for (index, track) in tracks.iter().enumerate() {
                if frozen[index] {
                    continue;
                }
                let weight = fraction_weight(track);
                if weight > 0.0 {
                    sizes[index] = finite_non_negative((unit * weight) as f32);
                }
            }
            break;
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests-src/ui/layout/grid/track_sizing_tests.rs"]
mod tests;

