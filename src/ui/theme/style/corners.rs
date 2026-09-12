//! 四角圆角的纯值契约。
//!
//! 与单值 `Style::border_radius` 表示同一个 `borderRadius` 属性的两种输入：
//! 显式单值（含 0）与四角值在声明层互相完整覆盖，不存在叠加；
//! 最终有效圆角由 [`crate::ui::theme::style::Style::effective_border_radius`]
//! 统一解析。归一化（相邻半径之和超出边长时按比例缩小）由 draw System 的
//! 公开 `Radius::normalized` 在绘制前统一执行，本类型不持有第二套规则。

/// 四个角的圆角半径，顺序为左上、右上、右下、左下。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CornerRadii {
    /// 左上角半径。
    pub tl: f32,
    /// 右上角半径。
    pub tr: f32,
    /// 右下角半径。
    pub br: f32,
    /// 左下角半径。
    pub bl: f32,
}

impl CornerRadii {
    /// 创建四角半径；调用方保证值为有限非负。
    pub const fn new(tl: f32, tr: f32, br: f32, bl: f32) -> Self {
        Self { tl, tr, br, bl }
    }

    /// 四角同值的便捷构造。
    pub const fn uniform(r: f32) -> Self {
        Self {
            tl: r,
            tr: r,
            br: r,
            bl: r,
        }
    }

    /// 全部直角。
    pub const fn zero() -> Self {
        Self::uniform(0.0)
    }

    /// 逐角收敛为有限非负值；非有限或负值按直角处理。
    pub fn sanitized(self) -> Self {
        fn corner(value: f32) -> f32 {
            if value.is_finite() && value > 0.0 {
                value
            } else {
                0.0
            }
        }
        Self {
            tl: corner(self.tl),
            tr: corner(self.tr),
            br: corner(self.br),
            bl: corner(self.bl),
        }
    }

    /// 是否全部角都是直角（含已收敛的非法值）。
    pub fn is_zero(self) -> bool {
        let s = self.sanitized();
        s.tl == 0.0 && s.tr == 0.0 && s.br == 0.0 && s.bl == 0.0
    }

    /// 转换为 draw System 的公开圆角值；不在此处归一化。
    pub fn to_radius(self) -> crate::draw::Radius {
        let s = self.sanitized();
        crate::draw::Radius {
            tl: s.tl,
            tr: s.tr,
            br: s.br,
            bl: s.bl,
        }
    }
}

impl From<f32> for CornerRadii {
    fn from(uniform: f32) -> Self {
        Self::uniform(uniform)
    }
}
