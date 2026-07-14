//! 通用浮层放置方向。

/// 浮层相对所属窗口逻辑客户区的放置位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Top,
    TopLeft,
    TopRight,
    Bottom,
    BottomLeft,
    BottomRight,
    Left,
    Right,
}

impl Placement {
    pub(crate) fn horizontal_start(self, available: f32, item_width: f32, inset: f32) -> f32 {
        let remaining = (available - item_width).max(0.0);
        match self {
            Self::TopLeft | Self::BottomLeft | Self::Left => inset.min(remaining),
            Self::TopRight | Self::BottomRight | Self::Right => (remaining - inset).max(0.0),
            Self::Top | Self::Bottom => remaining * 0.5,
        }
    }

    pub(crate) fn vertical_start(self, available: f32, stack_height: f32, inset: f32) -> f32 {
        let remaining = (available - stack_height).max(0.0);
        match self {
            Self::Top | Self::TopLeft | Self::TopRight => inset.min(remaining),
            Self::Bottom | Self::BottomLeft | Self::BottomRight => (remaining - inset).max(0.0),
            Self::Left | Self::Right => remaining * 0.5,
        }
    }
}
