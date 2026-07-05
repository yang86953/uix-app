//! 动画契约。

/// 可在线性空间中进行插值的类型。
pub trait Animatable: Clone + Copy + Send + 'static {
    fn lerp(from: Self, to: Self, t: f64) -> Self;
    fn delta(from: Self, to: Self) -> f64;
}
