//! 柱状数据条目。

use crate::draw::Color;

#[derive(Debug, Clone, PartialEq)]
/// 柱状图中的单个具名数值条目。
pub struct BarData {
    /// 该数据项在坐标轴或图例中显示的标签。
    pub label: String,
    /// 用于计算柱高的原始数值。
    pub value: f32,
    /// 该柱使用的绘制颜色。
    pub color: Color,
}

impl BarData {
    /// 创建具有标签、数值和显式颜色的柱状数据条目。
    pub fn new(label: impl Into<String>, value: f32, color: Color) -> Self {
        Self {
            label: label.into(),
            value,
            color,
        }
    }
}
