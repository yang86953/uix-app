//! 保留单位的样式长度、尺寸约束与百分比参照。
//!
//! 声明阶段只保存单位与数值，直到最终约束消费时才按参照轴解析为逻辑像素，
//! 不提前一律转成 f32 像素。

use crate::core::Size;

/// 保留声明单位的样式长度。
///
/// 支持集合：`auto`/`none`（不约束）、逻辑像素、参照轴百分比，以及
/// px 与百分比加减形成的有限 `calc()`。不接受任意函数、乘除或业务表达式。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum StyleLength {
    /// 不约束：min 的 `auto`、max 的 `none`、定位 inset 的 `auto`。
    #[default]
    Auto,
    /// 逻辑像素。
    Px(f32),
    /// 参照轴百分比；`50.0` 表示 50%。
    Percent(f32),
    /// 有限 `calc()`：`px + percent% × 参照轴`。
    Calc {
        /// 像素加法项，可为负。
        px: f32,
        /// 百分比加法项，可为负。
        percent: f32,
    },
}

impl StyleLength {
    /// 创建逻辑像素长度；拒绝无法形成稳定布局几何的数值。
    pub fn px(value: f32) -> Self {
        assert!(value.is_finite(), "StyleLength::px requires a finite value");
        Self::Px(value)
    }

    /// 创建百分比长度；`50.0` 表示 50%。
    pub fn percent(value: f32) -> Self {
        assert!(
            value.is_finite(),
            "StyleLength::percent requires a finite value"
        );
        Self::Percent(value)
    }

    /// 创建 px 与百分比加减形成的有限 `calc()`。
    pub fn calc(px: f32, percent: f32) -> Self {
        assert!(
            px.is_finite() && percent.is_finite(),
            "StyleLength::calc requires finite components"
        );
        Self::Calc { px, percent }
    }

    /// 是否不施加任何约束。
    pub const fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }

    /// 是否需要参照轴才能解析。
    pub const fn depends_on_reference(&self) -> bool {
        match self {
            Self::Auto | Self::Px(_) => false,
            Self::Percent(_) => true,
            Self::Calc { percent, .. } => *percent != 0.0,
        }
    }

    /// 按参照轴解析为逻辑像素。
    ///
    /// `Auto`、非有限分量，以及参照轴未定时的百分比项都返回 `None`（视为
    /// 未约束），不静默当作 0；`calc` 含百分比项且参照未定时整体视为未约束。
    pub fn resolve(self, reference: Option<f32>) -> Option<f32> {
        let reference = reference.filter(|value| value.is_finite() && *value < f32::MAX);
        let resolved = match self {
            Self::Auto => return None,
            Self::Px(px) => px,
            Self::Percent(percent) => reference? * percent / 100.0,
            Self::Calc { px, percent } => {
                if percent == 0.0 {
                    px
                } else {
                    px + reference? * percent / 100.0
                }
            }
        };
        (resolved.is_finite() && resolved.abs() < f32::MAX).then_some(resolved)
    }
}

impl From<f32> for StyleLength {
    /// 裸数值按逻辑像素解释，保持旧 `f32` 调用可编译；非有限值与
    /// [`StyleLength::px`] 一样拒绝，不静默退化为 auto。
    fn from(value: f32) -> Self {
        assert!(
            value.is_finite(),
            "StyleLength from f32 requires a finite value"
        );
        Self::Px(value)
    }
}

impl StyleLength {
    /// 校验 min/max 尺寸约束入口的合法集合：分量必须有限，单值 px/百分比必须
    /// 非负；负分量只能经 `calc` 表达（结果由求解钳到 0）。
    pub(crate) fn expect_size_bound(self, property: &str) -> Self {
        assert!(
            self.is_legal_size_bound(),
            "{property} requires a finite, non-negative px or percent value (use StyleLength::calc for negative components)"
        );
        self
    }

    /// 单值 px/百分比是否有限且非负、`calc` 分量是否有限；`Auto` 恒合法。
    pub(crate) fn is_legal_size_bound(self) -> bool {
        match self {
            Self::Auto => true,
            Self::Px(value) | Self::Percent(value) => value.is_finite() && value >= 0.0,
            Self::Calc { px, percent } => px.is_finite() && percent.is_finite(),
        }
    }
}

/// 百分比解析使用的父内容盒参照轴；`None` 表示该轴尺寸未定。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PercentReference {
    /// 水平参照（父内容盒宽度）。
    pub width: Option<f32>,
    /// 垂直参照（父内容盒高度）。
    pub height: Option<f32>,
}

impl PercentReference {
    /// 两个轴都未定。
    pub const NONE: Self = Self {
        width: None,
        height: None,
    };

    /// 分别指定两个轴的参照。
    pub const fn new(width: Option<f32>, height: Option<f32>) -> Self {
        Self { width, height }
    }

    /// 两个轴都由给定尺寸确定。
    pub fn definite(size: Size) -> Self {
        Self {
            width: Some(size.w),
            height: Some(size.h),
        }
    }
}

/// 四个方向的尺寸约束声明；`Auto` 表示该项不约束。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SizeConstraints {
    /// 最小宽度。
    pub min_width: StyleLength,
    /// 最大宽度。
    pub max_width: StyleLength,
    /// 最小高度。
    pub min_height: StyleLength,
    /// 最大高度。
    pub max_height: StyleLength,
}

impl SizeConstraints {
    /// 不施加任何约束。
    pub const NONE: Self = Self {
        min_width: StyleLength::Auto,
        max_width: StyleLength::Auto,
        min_height: StyleLength::Auto,
        max_height: StyleLength::Auto,
    };

    /// 是否没有任何约束。
    pub fn is_none(&self) -> bool {
        *self == Self::NONE
    }

    /// 逐项取本值，未声明项回退到 `fallback`。
    pub fn or(self, fallback: Self) -> Self {
        let pick = |own: StyleLength, other: StyleLength| if own.is_auto() { other } else { own };
        Self {
            min_width: pick(self.min_width, fallback.min_width),
            max_width: pick(self.max_width, fallback.max_width),
            min_height: pick(self.min_height, fallback.min_height),
            max_height: pick(self.max_height, fallback.max_height),
        }
    }

    /// 校验四项声明都在 min/max 合法集合内（单值 px/% 有限非负、calc 分量有限、
    /// Auto）；直接写入 `Style`/`SizeConstraints` 字段绕过便捷入口的非法值在此与
    /// 便捷入口同样被拒绝，不静默替换为 auto 或 0。
    pub fn expect_legal(self) -> Self {
        for (name, length) in [
            ("min_width", self.min_width),
            ("max_width", self.max_width),
            ("min_height", self.min_height),
            ("max_height", self.max_height),
        ] {
            length.expect_size_bound(name);
        }
        self
    }

    /// 按参照轴解析为 border-box 的最小与最大尺寸；消费前先经 [`Self::expect_legal`]
    /// 拒绝非法声明。
    ///
    /// 未约束的最小值为 0，最大值为 [`Size::infinite`]；`calc` 负结果收敛为 0；
    /// 参照未定的百分比项按不约束处理；同轴 min 大于 max 时 min 优先，max 抬高到 min。
    pub fn resolve(&self, reference: PercentReference) -> ResolvedSizeBounds {
        self.expect_legal();
        let lower = |length: StyleLength, axis: Option<f32>| {
            length.resolve(axis).map_or(0.0, |value| value.max(0.0))
        };
        let upper = |length: StyleLength, axis: Option<f32>, minimum: f32| {
            length
                .resolve(axis)
                .map_or(f32::MAX, |value| value.max(0.0).max(minimum))
        };
        let min_w = lower(self.min_width, reference.width);
        let min_h = lower(self.min_height, reference.height);
        ResolvedSizeBounds {
            min: Size::new(min_w, min_h),
            max: Size::new(
                upper(self.max_width, reference.width, min_w),
                upper(self.max_height, reference.height, min_h),
            ),
        }
    }
}

/// 已按参照轴解析的 border-box 尺寸区间。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedSizeBounds {
    /// 最小 border-box 尺寸。
    pub min: Size,
    /// 最大 border-box 尺寸；无上限用 `f32::MAX`。
    pub max: Size,
}

impl ResolvedSizeBounds {
    /// 不施加任何约束的区间。
    pub const UNBOUNDED: Self = Self {
        min: Size { w: 0.0, h: 0.0 },
        max: Size {
            w: f32::MAX,
            h: f32::MAX,
        },
    };

    /// 把尺寸钳入区间；非有限输入分量按 0 处理后再钳制。
    pub fn clamp(&self, size: Size) -> Size {
        let clamp_axis = |value: f32, minimum: f32, maximum: f32| {
            let value = if value.is_finite() && value < f32::MAX {
                value
            } else {
                0.0
            };
            value.max(minimum).min(maximum.max(minimum))
        };
        Size::new(
            clamp_axis(size.w, self.min.w, self.max.w),
            clamp_axis(size.h, self.min.h, self.max.h),
        )
    }
}

#[cfg(test)]
#[path = "../../../../tests-src/ui/theme/style/length_tests.rs"]
mod tests;

