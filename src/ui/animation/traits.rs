//! 动画契约。

/// 可在线性空间中进行插值的类型。
pub trait Animatable: Clone + Copy + Send + 'static {
    /// 返回从起点到终点按进度 `t` 插值得到的值。
    fn lerp(from: Self, to: Self, t: f64) -> Self;
    /// 返回两个值之间用于收敛判断的标量差异。
    fn delta(from: Self, to: Self) -> f64;
}
