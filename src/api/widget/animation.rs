//! # 动画契约
//!
//! 可插值类型 trait 与动画值类型 re-export。

/// 可在线性空间中进行插值的类型。
pub trait Animatable: Clone + Copy + Send + 'static {
    fn lerp(from: Self, to: Self, t: f64) -> Self;
    fn delta(from: Self, to: Self) -> f64;
}

// ── 动画类型 re-export ──
pub use crate::widget::animation::core::Animation;
pub use crate::widget::animation::easing::Easing;
pub use crate::widget::animation::transition::{presets, SlideDirection, Transition, TransitionPlayer};
