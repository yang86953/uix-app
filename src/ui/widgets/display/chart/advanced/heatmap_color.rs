//! 高级热力图的无状态色阶插值辅助。

// 引入热力图配色返回的绘制颜色值。
use crate::draw::Color;
// 复用动画层的颜色线性插值实现。
use crate::ui::animation::traits::Animatable;
// 复用父 Advanced Chart Module 唯一拥有的配置。
use super::ChartPlaceholder;

// 为高级图表补充只读取配置的热力图配色能力。
impl ChartPlaceholder {
    /// 热力图配色：色阶存在时按位置插值，否则在两端色之间线性插值。
    pub(crate) fn heatmap_color(&self, value: f32) -> Color {
        // 色阶不足两档时直接在两色之间插值（保持原 lerp_color 的 t 裁剪语义）。
        if self.color_stops.len() < 2 {
            return Animatable::lerp(
                self.color_min,
                self.color_max,
                f64::from(value.clamp(0.0, 1.0)),
            );
        }
        // 将输入约束到有效的归一化色阶范围。
        let value = value.clamp(0.0, 1.0);
        // 找到 value 所在的相邻色阶区间。
        let Some(window) = self
            .color_stops
            .windows(2)
            .find(|window| value <= window[1].0)
        else {
            // 超出最大色阶位置时取末档颜色。
            return self
                .color_stops
                .last()
                .map_or(self.color_max, |(_, color)| *color);
        };
        // 以最小正跨度避免重复色阶位置导致除零。
        let span = (window[1].0 - window[0].0).max(f32::EPSILON);
        // 在命中的相邻颜色之间按局部比例插值（value 低于首档位置时比例为负，需裁剪）。
        Animatable::lerp(
            window[0].1,
            window[1].1,
            f64::from(((value - window[0].0) / span).clamp(0.0, 1.0)),
        )
    }
}
