// 把一个轴的 min/max 收敛为有序且可用于实际布局的区间。
pub(super) fn normalized_axis_bounds(minimum: f32, maximum: f32) -> (f32, f32) {
    // 最小值只接受有限非负实际尺寸。
    let minimum = super::finite_non_negative(minimum);
    // 有限且不是无界哨兵的最大值继续参与钳制。
    let maximum = if maximum.is_finite() && maximum.abs() < f32::MAX {
        // 最大值不得低于零或已经归一的最小值。
        maximum.max(0.0).max(minimum)
    } else {
        // 无界或非法最大值统一回退为内部无界上限。
        f32::MAX
    };
    // 返回不会触发反向区间的上下限。
    (minimum, maximum)
}
