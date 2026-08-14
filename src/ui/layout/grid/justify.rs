// 拆分自 grid.rs：整组列轨的水平内容对齐（justify-content）。
// 引入轨道类型与内容对齐声明。
use super::{GridTrack, JustifyContent};
// 引入有限化辅助（轨道尺寸求解模块提供）。
use super::track_sizing::finite_non_negative;

pub(super) fn justify_grid_content(
    // 原始轨道类型用于 Stretch 识别 Auto 列。
    tracks: &[GridTrack],
    // 已解析的实际列宽，可由 Stretch 原位扩展。
    sizes: &mut [f32],
    // 父级内容区域的可用宽度。
    available: f32,
    // 作者声明的全部基础列间距。
    total_gap: f32,
    // 整组列轨的水平内容对齐模式。
    justify: JustifyContent,
) -> (f32, f32, bool) {
    // 使用 f64 汇总列宽，避免多轨道累加提前溢出。
    let used = sizes
        // 遍历已经有限化的实际列宽。
        .iter()
        // 把每条列宽提升到 f64 账本。
        .map(|size| finite_non_negative(*size) as f64)
        // 汇总全部列轨宽度。
        .sum::<f64>();
    // 内容对齐只消费固定轨道与基础 gap 之后的正剩余空间。
    let free = (finite_non_negative(available) as f64
        // 扣除作者声明的全部基础列间距。
        - finite_non_negative(total_gap) as f64
        // 扣除已经解析的列轨宽度。
        - used)
        // 负剩余空间保持现有溢出几何，不反向分布。
        .max(0.0);
    // 没有正剩余空间时所有模式都保持既有轨道位置。
    if free <= 0.0 {
        // 不增加起点偏移、列间距或可用宽度占用。
        return (0.0, 0.0, false);
    }

    // 按声明模式换算起点偏移与相邻轨道附加间距。
    match justify {
        // Start 保持轨道组贴近内容起点。
        JustifyContent::Start => (0.0, 0.0, false),
        // Center 在前后各保留一半剩余空间。
        JustifyContent::Center => (finite_non_negative((free * 0.5) as f32), 0.0, true),
        // End 把全部剩余空间放到轨道组之前。
        JustifyContent::End => (finite_non_negative(free as f32), 0.0, true),
        // SpaceBetween 只在至少两条轨道时增加内部间距。
        JustifyContent::SpaceBetween if sizes.len() > 1 => (
            // 两端不增加偏移。
            0.0,
            // 全部剩余空间均分给相邻轨道间隙。
            finite_non_negative((free / sizes.len().saturating_sub(1) as f64) as f32),
            // 分布后内容占用完整可用宽度。
            true,
        ),
        // 单轨 SpaceBetween 与 Start 等价。
        JustifyContent::SpaceBetween => (0.0, 0.0, false),
        // SpaceAround 为每条轨道分配一份环绕空间。
        JustifyContent::SpaceAround if !sizes.is_empty() => {
            // 相邻轨道之间获得一整份空间。
            let distributed_gap = finite_non_negative((free / sizes.len() as f64) as f32);
            // 两端各保留半份空间。
            (distributed_gap * 0.5, distributed_gap, true)
        }
        // 空轨道不会进入正常布局，但仍提供确定性回退。
        JustifyContent::SpaceAround => (0.0, 0.0, false),
        // SpaceEvenly 在两端与每个相邻轨道之间使用等份空间。
        JustifyContent::SpaceEvenly => {
            // n 条轨道共有 n+1 份均匀空间。
            let distributed_gap =
                finite_non_negative((free / sizes.len().saturating_add(1) as f64) as f32);
            // 起点与内部附加间距使用同一份宽度。
            (distributed_gap, distributed_gap, true)
        }
        // Stretch 只扩展内容尺寸轨道，不改写 Px 或 Fr。
        JustifyContent::Stretch => {
            // 统计可以吸收剩余空间的 Auto 列数。
            let auto_count = tracks
                // 遍历原始轨道定义。
                .iter()
                // 只有 Auto 参与 Grid 内容 Stretch。
                .filter(|track| matches!(track, GridTrack::Auto))
                // 得到均分剩余空间的轨道数量。
                .count();
            // 没有 Auto 时 Stretch 按 Start 回退。
            if auto_count == 0 {
                // 固定与 Fr 轨道保持原尺寸和起点。
                return (0.0, 0.0, false);
            }
            // 把全部剩余空间均分给 Auto 列。
            let share = finite_non_negative((free / auto_count as f64) as f32);
            // 原位扩展每一条 Auto 列。
            for (index, track) in tracks.iter().enumerate() {
                // 固定与 Fr 轨道不参与 Stretch。
                if !matches!(track, GridTrack::Auto) {
                    // 跳过非 Auto 列。
                    continue;
                }
                // 使用有限化加法更新实际列宽。
                sizes[index] = finite_non_negative(sizes[index] + share);
            }
            // Auto 扩展后轨道组占用完整可用宽度。
            (0.0, 0.0, true)
        }
    }
}
