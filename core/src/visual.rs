// ============================================================================
// core/visual.rs — 视觉渲染基础类型
//
// 渲染无关的视觉原语（圆角、对齐、混合模式、渐变方向）。
// graphics 层和 ui 层共享，无需跨层依赖。
// ============================================================================

/// 四角圆角半径。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radius {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

impl Radius {
    pub const fn uniform(r: f32) -> Self {
        Self { tl: r, tr: r, br: r, bl: r }
    }

    pub const fn zero() -> Self {
        Self::uniform(0.0)
    }
}

impl Default for Radius {
    fn default() -> Self {
        Self::zero()
    }
}

/// 水平对齐。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// 垂直对齐。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    #[default]
    Top,
    Middle,
    Bottom,
    Baseline,
}

/// 像素混合模式。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    #[default]
    Alpha,
    SrcOver,
    Additive,
}

/// 渐变方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientDirection {
    Horizontal,
    Vertical,
    DiagonalTLBR,
    DiagonalBLTR,
}
