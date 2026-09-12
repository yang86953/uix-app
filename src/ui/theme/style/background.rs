//! 背景图层的纯值契约。

// 引入背景图颜色端点值。
use super::ColorValue;
// 引入背景尺寸解析使用的二维尺寸。
use crate::core::Size;

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
    /// 带角度和已定位色标的线性渐变；2..=16 项，位置非递减且在 0..=1。
    LinearGradientStops {
        /// 0° 向上，90° 向右；接受任意有限角度。
        angle_degrees: f32,
        /// 色标顺序；相同位置后项覆盖前项。
        stops: Vec<GradientStopValue>,
    },
    /// 使用从中心向外的双色径向渐变。
    RadialGradient {
        /// 渐变中心颜色。
        inner: ColorValue,
        /// 渐变外缘颜色。
        outer: ColorValue,
    },
}

/// 可解析主题颜色的线性渐变色标。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStopValue {
    /// 渐变轴上的位置，合法范围 0..=1。
    pub offset: f32,
    /// 在绘制时解析的主题颜色或具体颜色。
    pub color: ColorValue,
}
impl GradientStopValue {
    /// 创建待验证色标；绘制入口统一验证完整列表。
    pub const fn new(offset: f32, color: ColorValue) -> Self { Self { offset, color } }
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

// ════════════════════════════════════════════════════════════════════════════
// 背景尺寸（S4）
// ════════════════════════════════════════════════════════════════════════════

/// 背景图单轴尺寸。
///
/// `Auto` 轴保持图片固有比例：另一轴给出显式尺寸时按该轴的缩放比例
/// 推导本轴；两轴都是 `Auto` 时使用固有尺寸（与未声明 `backgroundSize`
/// 完全一致）。百分比参照当前绘制的 border-box（与背景定位共用同一盒）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackgroundAxisSize {
    /// 按固有比例或固有尺寸解析。
    Auto,
    /// 显式逻辑像素尺寸。
    Px(f32),
    /// 相对背景盒同轴尺寸的比例，合法范围 0.0 到 1.0 之外的值由解析层拒绝。
    Percent(f32),
}

/// 背景图整体尺寸策略；只作用于图片来源，渐变始终填满背景盒。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackgroundSize {
    /// 固有尺寸（默认值，也是显式恢复关键字 `auto`）。
    Auto,
    /// 等比缩放到完全覆盖背景盒，允许单侧溢出（剩余空间可为负）。
    Cover,
    /// 等比缩放到完整包含在背景盒内，允许单侧留白。
    Contain,
    /// 逐轴显式尺寸；单轴 `Auto` 时按另一轴比例保持宽高比，两轴显式时允许拉伸。
    Explicit(BackgroundAxisSize, BackgroundAxisSize),
}

impl Default for BackgroundSize {
    fn default() -> Self {
        Self::Auto
    }
}

impl BackgroundSize {
    /// 显式两轴尺寸的便捷构造。
    pub const fn explicit(width: BackgroundAxisSize, height: BackgroundAxisSize) -> Self {
        Self::Explicit(width, height)
    }

    /// 按背景盒与图片固有尺寸解析为最终平铺尺寸。
    ///
    /// 返回 `None` 表示无有效输入或显式零轴尺寸；调用方不产生图层。
    pub(crate) fn resolve_tile(self, box_size: Size, image_size: Size) -> Option<Size> {
        // 外层 None 是无效值，内层 None 才是 Auto；显式零必须保留为零。
        let scale_of = |axis: BackgroundAxisSize, box_axis: f32, image_axis: f32| -> Option<Option<f32>> {
            match axis {
                BackgroundAxisSize::Auto => Some(None),
                BackgroundAxisSize::Px(value) if value.is_finite() && value >= 0.0 =>
                    Some(Some(value / image_axis)),
                BackgroundAxisSize::Percent(value) if value.is_finite() && value >= 0.0 =>
                    Some(Some(box_axis * value / image_axis)),
                _ => None,
            }
        };
        if !box_size.w.is_finite()
            || !box_size.h.is_finite()
            || box_size.w <= 0.0
            || box_size.h <= 0.0
            || !image_size.w.is_finite()
            || !image_size.h.is_finite()
            || image_size.w <= 0.0
            || image_size.h <= 0.0
        {
            return None;
        }
        match self {
            Self::Auto => Some(image_size),
            Self::Cover => {
                let scale = (box_size.w / image_size.w).max(box_size.h / image_size.h);
                Some(Size::new(image_size.w * scale, image_size.h * scale))
            }
            Self::Contain => {
                let scale = (box_size.w / image_size.w).min(box_size.h / image_size.h);
                Some(Size::new(image_size.w * scale, image_size.h * scale))
            }
            Self::Explicit(width_axis, height_axis) => {
                let width_scale = scale_of(width_axis, box_size.w, image_size.w)?;
                let height_scale = scale_of(height_axis, box_size.h, image_size.h)?;
                let (scale_x, scale_y) = match (width_scale, height_scale) {
                    // 两轴显式：允许非等比拉伸。
                    (Some(sx), Some(sy)) => (sx, sy),
                    // 单轴显式：另一轴按同一比例保持固有宽高比。
                    (Some(sx), None) => (sx, sx),
                    (None, Some(sy)) => (sy, sy),
                    // 双 Auto 与 Self::Auto 等价。
                    (None, None) => (1.0, 1.0),
                };
                let tile = Size::new(image_size.w * scale_x, image_size.h * scale_y);
                (tile.w.is_finite() && tile.h.is_finite() && tile.w > 0.0 && tile.h > 0.0).then_some(tile)
            }
        }
    }
}
