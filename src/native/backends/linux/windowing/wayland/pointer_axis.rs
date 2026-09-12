//! Wayland 指针轴帧到平台中立滚轮步长的归一化 Component。

// libinput/Wayland 连续轴的一个传统滚轮刻度通常约为 15 个 surface 单位；
// 离散轴存在时以协议给出的刻度为权威，连续值只服务触控板与旧 compositor。
const CONTINUOUS_UNITS_PER_STEP: f64 = 15.0;

/// 保存一个 `wl_pointer.frame` 内尚未提交的两轴滚动事实。
#[derive(Debug, Default)]
pub(crate) struct PointerAxisFrame {
    // 连续水平距离在同一协议帧内累加。
    continuous_x: f64,
    // 连续垂直距离在同一协议帧内累加。
    continuous_y: f64,
    // 离散水平刻度存在时覆盖连续距离换算。
    discrete_x: Option<i32>,
    // 离散垂直刻度存在时覆盖连续距离换算。
    discrete_y: Option<i32>,
    // 高精度水平刻度以 120 为一个传统滚轮步长。
    value120_x: Option<i32>,
    // 高精度垂直刻度以 120 为一个传统滚轮步长。
    value120_y: Option<i32>,
}

impl PointerAxisFrame {
    /// 记录 Wayland 连续轴距离。
    pub(crate) fn record_continuous(&mut self, horizontal: bool, value: f64) {
        // 非有限协议值不能进入平台中立事件。
        if !value.is_finite() {
            return;
        }
        if horizontal {
            self.continuous_x += value;
        } else {
            self.continuous_y += value;
        }
    }

    /// 记录 Wayland 离散滚轮刻度。
    pub(crate) fn record_discrete(&mut self, horizontal: bool, steps: i32) {
        // 同一帧同一轴可能分批到达，必须保持总刻度。
        let slot = if horizontal {
            &mut self.discrete_x
        } else {
            &mut self.discrete_y
        };
        *slot = Some(slot.unwrap_or(0).saturating_add(steps));
    }

    /// 记录 Wayland v8 的高精度 120 基准滚轮刻度。
    pub(crate) fn record_value120(&mut self, horizontal: bool, value120: i32) {
        let slot = if horizontal {
            &mut self.value120_x
        } else {
            &mut self.value120_y
        };
        *slot = Some(slot.unwrap_or(0).saturating_add(value120));
    }

    /// 消费当前协议帧并返回统一的逻辑滚轮步长。
    pub(crate) fn take_normalized(&mut self) -> Option<(f32, f32)> {
        // 离散事实优先；触控板等无离散事件时把连续距离换算为同一单位。
        let x = normalized_axis(self.value120_x, self.discrete_x, self.continuous_x);
        let y = normalized_axis(self.value120_y, self.discrete_y, self.continuous_y);
        // 消费后立即复位，禁止跨 `wl_pointer.frame` 累积。
        *self = Self::default();
        let x = x as f32;
        let y = y as f32;
        // 精确零帧不制造空 Wheel 事件。
        (x != 0.0 || y != 0.0).then_some((x, y))
    }
}

// 把三代 Wayland 轴事实收敛为唯一逻辑步长，优先采用精度最高的协议值。
fn normalized_axis(value120: Option<i32>, discrete: Option<i32>, continuous: f64) -> f64 {
    value120.map_or_else(
        || discrete.map_or_else(|| continuous / CONTINUOUS_UNITS_PER_STEP, f64::from),
        |value| f64::from(value) / 120.0,
    )
}

#[cfg(test)]
#[path = "../../../../../../tests-src/native/backends/linux/windowing/wayland/pointer_axis_tests.rs"]
mod tests;

