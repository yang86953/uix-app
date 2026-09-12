//! 通用浮层放置方向。

/// 浮层相对所属窗口逻辑客户区的放置位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// 沿客户区顶部居中放置。
    Top,
    /// 沿客户区左上角放置。
    TopLeft,
    /// 沿客户区右上角放置。
    TopRight,
    /// 沿客户区底部居中放置。
    Bottom,
    /// 沿客户区左下角放置。
    BottomLeft,
    /// 沿客户区右下角放置。
    BottomRight,
    /// 沿客户区左侧垂直居中放置。
    Left,
    /// 沿客户区右侧垂直居中放置。
    Right,
}

impl Placement {
    pub fn horizontal_start(self, available: f32, item_width: f32, inset: f32) -> f32 {
        let remaining = (available - item_width).max(0.0);
        match self {
            Self::TopLeft | Self::BottomLeft | Self::Left => inset.min(remaining),
            Self::TopRight | Self::BottomRight | Self::Right => (remaining - inset).max(0.0),
            Self::Top | Self::Bottom => remaining * 0.5,
        }
    }

    pub fn vertical_start(self, available: f32, stack_height: f32, inset: f32) -> f32 {
        let remaining = (available - stack_height).max(0.0);
        match self {
            Self::Top | Self::TopLeft | Self::TopRight => inset.min(remaining),
            Self::Bottom | Self::BottomLeft | Self::BottomRight => (remaining - inset).max(0.0),
            Self::Left | Self::Right => remaining * 0.5,
        }
    }
}
