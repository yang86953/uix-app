//! 柱状数据条目。

use crate::draw::Color;

#[derive(Debug, Clone, PartialEq)]
pub struct BarData {
    pub label: String,
    pub value: f32,
    pub color: Color,
}

impl BarData {
    pub fn new(label: impl Into<String>, value: f32, color: Color) -> Self {
        Self {
            label: label.into(),
            value,
            color,
        }
    }
}
