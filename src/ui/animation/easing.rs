//! Easing — 缓动曲线，支持 CSS cubic-bezier 以及经典预设。

use crate::draw::Color;
use crate::ui::animation::traits::Animatable;

// ════════════════════════════════════════════════════════════════════════════
// Easing 曲线
// ════════════════════════════════════════════════════════════════════════════

/// 缓动函数；输入会收敛到 [0, 1]，Back / Elastic 曲线允许输出短暂越界。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    /// 线性插值。
    Linear,
    /// 二次方缓入 `t²`。
    QuadIn,
    /// 二次方缓出 `t(2−t)`。
    QuadOut,
    /// 二次方缓入缓出。
    QuadInOut,
    /// 三次方缓入 `t³`。
    CubicIn,
    /// 三次方缓出。
    CubicOut,
    /// 三次方缓入缓出。
    CubicInOut,
    /// 四次方缓入 `t⁴`。
    QuartIn,
    /// 四次方缓出。
    QuartOut,
    /// 四次方缓入缓出。
    QuartInOut,
    /// 弹性缓出（超过目标然后回弹）。
    BackOut,
    /// 弹性缓入。
    BackIn,
    /// 弹跳缓出。
    Bounce,
    /// 弹性缓出，允许短暂越过目标值。
    Elastic,
    /// 自定义三次贝塞尔曲线 `(x1, y1, x2, y2)`。
    CubicBezier(f64, f64, f64, f64),
}

#[allow(non_upper_case_globals)]
impl Easing {
    /// 线性缓动的公开链式写法。
    pub const linear: Self = Self::Linear;
    /// 二次方缓入。
    pub const ease_in: Self = Self::QuadIn;
    /// 二次方缓出。
    pub const ease_out: Self = Self::QuadOut;
    /// 二次方缓入缓出。
    pub const ease_in_out: Self = Self::QuadInOut;
    /// 三次方缓入。
    pub const cubic_in: Self = Self::CubicIn;
    /// 三次方缓出。
    pub const cubic_out: Self = Self::CubicOut;
    /// 三次方缓入缓出。
    pub const cubic_in_out: Self = Self::CubicInOut;
    /// 分段弹跳缓出。
    pub const bounce: Self = Self::Bounce;
    /// 衰减振荡缓出。
    pub const elastic: Self = Self::Elastic;

    /// 构造自定义三次贝塞尔曲线。
    pub const fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self::CubicBezier(x1, y1, x2, y2)
    }

    /// 将 `t ∈ [0,1]` 映射到缓动后的值。
    pub fn sample(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::QuadIn => t * t,
            Self::QuadOut => t * (2.0 - t),
            Self::QuadInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
            Self::CubicIn => t * t * t,
            Self::CubicOut => {
                let t = t - 1.0;
                t * t * t + 1.0
            }
            Self::CubicInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let t = 2.0 * t - 2.0;
                    0.5 * t * t * t + 1.0
                }
            }
            Self::QuartIn => t * t * t * t,
            Self::QuartOut => {
                let t = t - 1.0;
                -(t * t * t * t) + 1.0
            }
            Self::QuartInOut => {
                if t < 0.5 {
                    8.0 * t * t * t * t
                } else {
                    let t = t - 1.0;
                    -8.0 * t * t * t * t + 1.0
                }
            }
            Self::BackIn => {
                const C1: f64 = 1.70158;
                const C3: f64 = C1 + 1.0;
                C3 * t * t * t - C1 * t * t
            }
            Self::BackOut => {
                const C1: f64 = 1.70158;
                const C3: f64 = C1 + 1.0;
                let t = t - 1.0;
                C3 * t * t * t + C1 * t * t + 1.0
            }
            Self::Bounce => sample_bounce_out(t),
            Self::Elastic => sample_elastic_out(t),
            Self::CubicBezier(x1, y1, x2, y2) => sample_cubic_bezier(*x1, *y1, *x2, *y2, t),
        }
    }

    // ── 标准 CSS 缓动预设──

    /// CSS 默认缓动: `cubic-bezier(0.25, 0.1, 0.25, 1)`
    pub const fn css_ease() -> Self {
        Self::CubicBezier(0.25, 0.1, 0.25, 1.0)
    }

    /// CSS 缓入: `cubic-bezier(0.42, 0, 1, 1)`
    pub const fn css_ease_in() -> Self {
        Self::CubicBezier(0.42, 0.0, 1.0, 1.0)
    }

    /// CSS 缓出: `cubic-bezier(0, 0, 0.58, 1)`
    pub const fn css_ease_out() -> Self {
        Self::CubicBezier(0.0, 0.0, 0.58, 1.0)
    }

    /// CSS 缓入缓出: `cubic-bezier(0.42, 0, 0.58, 1)`
    pub const fn css_ease_in_out() -> Self {
        Self::CubicBezier(0.42, 0.0, 0.58, 1.0)
    }
}

fn sample_bounce_out(t: f64) -> f64 {
    const N1: f64 = 7.5625;
    const D1: f64 = 2.75;
    if t < 1.0 / D1 {
        N1 * t * t
    } else if t < 2.0 / D1 {
        let t = t - 1.5 / D1;
        N1 * t * t + 0.75
    } else if t < 2.5 / D1 {
        let t = t - 2.25 / D1;
        N1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / D1;
        N1 * t * t + 0.984375
    }
}

fn sample_elastic_out(t: f64) -> f64 {
    if t == 0.0 || t == 1.0 {
        return t;
    }
    const ANGULAR_SCALE: f64 = std::f64::consts::TAU / 3.0;
    2.0_f64.powf(-10.0 * t) * ((10.0 * t - 0.75) * ANGULAR_SCALE).sin() + 1.0
}

impl Default for Easing {
    fn default() -> Self {
        Self::css_ease()
    }
}

// ── Cubic Bezier 采样 ──────────────────────────────────────────────

/// 使用牛顿法（8 次迭代）+ 二分法回退求解三次贝塞尔曲线 `t` 对应的 `x` 值，再求 `y`。
fn sample_cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, t: f64) -> f64 {
    let mut guess = t;
    let mut converged = true;
    for _ in 0..8 {
        let x = cubic_bezier_x(x1, x2, guess);
        let dx = cubic_bezier_dx(x1, x2, guess);
        if dx.abs() < 1e-12 {
            converged = false;
            break;
        }
        let step = (x - t) / dx;
        guess -= step;
        if step.abs() < 1e-10 {
            break;
        }
    }
    if !converged || !(0.0..=1.0).contains(&guess) {
        // 二分法回退
        let mut lo = 0.0;
        let mut hi = 1.0;
        for _ in 0..12 {
            guess = (lo + hi) * 0.5;
            let x = cubic_bezier_x(x1, x2, guess);
            if (x - t).abs() < 1e-10 {
                break;
            }
            if x < t {
                lo = guess;
            } else {
                hi = guess;
            }
        }
    }
    guess = guess.clamp(0.0, 1.0);
    cubic_bezier_y(y1, y2, guess)
}

fn cubic_bezier_x(x1: f64, x2: f64, t: f64) -> f64 {
    let u = 1.0 - t;
    3.0 * u * u * t * x1 + 3.0 * u * t * t * x2 + t * t * t
}

fn cubic_bezier_y(y1: f64, y2: f64, t: f64) -> f64 {
    let u = 1.0 - t;
    3.0 * u * u * t * y1 + 3.0 * u * t * t * y2 + t * t * t
}

fn cubic_bezier_dx(x1: f64, x2: f64, t: f64) -> f64 {
    let u = 1.0 - t;
    3.0 * u * u * x1 + 6.0 * u * t * (x2 - x1) + 3.0 * t * t * (1.0 - x2)
}

// Animatable trait 定义已迁移至 ui::traits::animation，以下是类型实现。

impl Animatable for f32 {
    fn lerp(from: Self, to: Self, t: f64) -> Self {
        from + (to - from) * t as f32
    }
    fn delta(from: Self, to: Self) -> f64 {
        (to - from) as f64
    }
}

impl Animatable for f64 {
    fn lerp(from: Self, to: Self, t: f64) -> Self {
        from + (to - from) * t
    }
    fn delta(from: Self, to: Self) -> f64 {
        to - from
    }
}

impl Animatable for Color {
    fn lerp(from: Self, to: Self, t: f64) -> Self {
        let t = t as f32;
        Color::from_rgba(
            (from.r as f32 + (to.r as f32 - from.r as f32) * t).round() as u8,
            (from.g as f32 + (to.g as f32 - from.g as f32) * t).round() as u8,
            (from.b as f32 + (to.b as f32 - from.b as f32) * t).round() as u8,
            (from.a as f32 + (to.a as f32 - from.a as f32) * t).round() as u8,
        )
    }
    fn delta(from: Self, to: Self) -> f64 {
        let dr = to.r as f64 - from.r as f64;
        let dg = to.g as f64 - from.g as f64;
        let db = to.b as f64 - from.b as f64;
        let da = to.a as f64 - from.a as f64;
        (dr * dr + dg * dg + db * db + da * da).sqrt()
    }
}

impl Animatable for crate::core::Rect {
    fn lerp(from: Self, to: Self, t: f64) -> Self {
        let t = t as f32;
        Self {
            x: from.x + (to.x - from.x) * t,
            y: from.y + (to.y - from.y) * t,
            w: from.w + (to.w - from.w) * t,
            h: from.h + (to.h - from.h) * t,
        }
    }
    fn delta(from: Self, to: Self) -> f64 {
        let dx = to.x as f64 - from.x as f64;
        let dy = to.y as f64 - from.y as f64;
        let dw = to.w as f64 - from.w as f64;
        let dh = to.h as f64 - from.h as f64;
        (dx * dx + dy * dy + dw * dw + dh * dh).sqrt()
    }
}

impl Animatable for crate::core::Point {
    fn lerp(from: Self, to: Self, t: f64) -> Self {
        let t = t as f32;
        Self {
            x: from.x + (to.x - from.x) * t,
            y: from.y + (to.y - from.y) * t,
        }
    }
    fn delta(from: Self, to: Self) -> f64 {
        let dx = to.x as f64 - from.x as f64;
        let dy = to.y as f64 - from.y as f64;
        (dx * dx + dy * dy).sqrt()
    }
}
