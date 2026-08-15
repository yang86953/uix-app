// 引入运行时布局帧与已解析原点坐标。
use crate::core::{Point, Rect};

/// 描述单轴变换原点相对于布局帧的取值方式。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformOriginValue {
    /// 以布局帧尺寸比例表示；`0.5` 表示该轴中心。
    Fraction(f32),
    /// 以布局帧起点为基准的固定像素偏移。
    Pixels(f32),
}

impl TransformOriginValue {
    /// 构造以零到一比例表达的原点轴值。
    pub const fn fraction(value: f32) -> Self {
        // 保存比例原值，允许调用方表达帧外原点。
        Self::Fraction(value)
    }

    /// 构造以布局帧起点为基准的像素轴值。
    pub const fn pixels(value: f32) -> Self {
        // 保存固定像素偏移，允许负值与帧外原点。
        Self::Pixels(value)
    }

    // 在布局帧已知后解析单轴绝对坐标。
    fn resolve(self, start: f32, extent: f32) -> f32 {
        // 按取值方式选择相对尺寸或固定像素语义。
        match self {
            // 比例值随当前布局帧尺寸变化。
            Self::Fraction(value) => start + extent * value,
            // 像素值只相对当前布局帧起点。
            Self::Pixels(value) => start + value,
        }
    }
}

/// 描述二维仿射变换围绕布局帧应用的原点。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformOrigin {
    // 保存水平方向原点语义。
    horizontal: TransformOriginValue,
    // 保存垂直方向原点语义。
    vertical: TransformOriginValue,
}

impl TransformOrigin {
    /// 构造独立水平与垂直轴值的二维原点。
    pub const fn new(
        // 接收水平方向轴值。
        horizontal: TransformOriginValue,
        // 接收垂直方向轴值。
        vertical: TransformOriginValue,
    ) -> Self {
        // 保存两个轴的公开值语义。
        Self {
            // 保存水平轴值。
            horizontal,
            // 保存垂直轴值。
            vertical,
        }
    }

    /// 返回布局帧中心原点。
    pub const fn center() -> Self {
        // 两轴各使用一半帧尺寸。
        Self::new(
            // 水平中心。
            TransformOriginValue::Fraction(0.5),
            // 垂直中心。
            TransformOriginValue::Fraction(0.5),
        )
    }

    // 在布局帧确定后解析为绝对二维坐标。
    pub(crate) fn resolve(self, frame: Rect) -> Point {
        // 分别解析两个轴并组装最终原点。
        Point::new(
            // 水平轴相对布局帧左侧解析。
            self.horizontal.resolve(frame.x, frame.w),
            // 垂直轴相对布局帧顶部解析。
            self.vertical.resolve(frame.y, frame.h),
        )
    }
}

impl Default for TransformOrigin {
    // 默认遵循 UIX/CSS 的布局帧中心语义。
    fn default() -> Self {
        // 返回稳定中心值。
        Self::center()
    }
}
