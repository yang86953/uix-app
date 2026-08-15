//! 背景图层的纯值契约。

// 引入背景图颜色端点值。
use super::ColorValue;

/// 单层背景图来源。
#[derive(Debug, Clone, PartialEq)]
pub enum BackgroundImage {
    /// 显式关闭背景图。
    None,
    /// 使用本地图片路径。
    Url(String),
    /// 使用上下方向的双色线性渐变。
    LinearGradient {
        /// 渐变起始颜色。
        start: ColorValue,
        /// 渐变结束颜色。
        end: ColorValue,
    },
    /// 使用从中心向外的双色径向渐变。
    RadialGradient {
        /// 渐变中心颜色。
        inner: ColorValue,
        /// 渐变外缘颜色。
        outer: ColorValue,
    },
}

/// 单轴背景定位值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackgroundAxisPosition {
    /// 对齐轴起点。
    Start,
    /// 对齐轴中心。
    Center,
    /// 对齐轴终点。
    End,
    /// 从轴起点偏移固定像素。
    Pixels(f32),
    /// 按剩余空间比例偏移，合法范围为零到一。
    Percent(f32),
}

// 为单轴背景定位提供确定默认值。
impl Default for BackgroundAxisPosition {
    // 默认对齐轴起点。
    fn default() -> Self {
        // 返回 CSS 的零百分比语义。
        Self::Start
    }
}

// 解析背景图相对剩余空间的单轴偏移。
impl BackgroundAxisPosition {
    // 将定位值解析为可直接加到容器起点的偏移。
    pub(crate) fn resolve(self, available: f32) -> f32 {
        // 非有限剩余空间不应传播到绘制几何。
        if !available.is_finite() {
            // 无效几何回退到轴起点。
            return 0.0;
        }
        // 按闭合定位种类解析偏移。
        match self {
            // 起点没有额外偏移。
            Self::Start => 0.0,
            // 中点使用一半剩余空间。
            Self::Center => available * 0.5,
            // 终点使用全部剩余空间。
            Self::End => available,
            // 只接受有限像素值。
            Self::Pixels(value) if value.is_finite() => value,
            // 非有限像素值安全回退到起点。
            Self::Pixels(_) => 0.0,
            // 百分比按剩余空间计算并限制在合法范围。
            Self::Percent(value) if value.is_finite() => available * value.clamp(0.0, 1.0),
            // 非有限百分比安全回退到起点。
            Self::Percent(_) => 0.0,
        }
    }
}

/// 二维背景定位。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BackgroundPosition {
    /// 水平轴定位。
    pub x: BackgroundAxisPosition,
    /// 垂直轴定位。
    pub y: BackgroundAxisPosition,
}

// 提供明确的二维背景定位构造器。
impl BackgroundPosition {
    /// 从两个轴值创建背景定位。
    pub const fn new(x: BackgroundAxisPosition, y: BackgroundAxisPosition) -> Self {
        // 保存已经解析为类型值的两个轴声明。
        Self { x, y }
    }
}

/// 单层背景图重复方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackgroundRepeat {
    /// 同时沿两个轴重复。
    #[default]
    Repeat,
    /// 只沿水平轴重复。
    RepeatX,
    /// 只沿垂直轴重复。
    RepeatY,
    /// 两个轴都不重复。
    NoRepeat,
}

// 将公开重复枚举转换为绘制轴开关。
impl BackgroundRepeat {
    // 返回是否沿水平轴重复。
    pub(crate) fn repeats_x(self) -> bool {
        // 只有 repeat 与 repeat-x 开启水平重复。
        matches!(self, Self::Repeat | Self::RepeatX)
    }

    // 返回是否沿垂直轴重复。
    pub(crate) fn repeats_y(self) -> bool {
        // 只有 repeat 与 repeat-y 开启垂直重复。
        matches!(self, Self::Repeat | Self::RepeatY)
    }
}
