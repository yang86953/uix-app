//! 动画系统 — 缓动函数 + 数值插值。
//!
//! 提供标准 CSS 缓动函数和泛型插值器，用于 UI 动画。

use std::time::Instant;

/// 缓动函数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    /// 缓入（加速）。
    EaseIn,
    /// 缓出（减速）。
    EaseOut,
    /// 缓入缓出（先加速后减速）。
    EaseInOut,
    /// 弹跳效果（超出目标再回弹）。
    Bounce,
    /// 使用自定义三次贝塞尔曲线。(x1, y1, x2, y2)，控制点 P1 和 P2。
    CubicBezier(f32, f32, f32, f32),
}

impl Easing {
    /// 将 t (0~1) 映射为缓动后的值 (0~1)。
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t * t,
            Easing::EaseOut => 1.0 - (1.0 - t).powi(3),
            Easing::EaseInOut => {
                if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
            }
            Easing::Bounce => {
                let n1 = 7.5625;
                let d1 = 2.75;
                if t < 1.0 / d1 { n1 * t * t }
                else if t < 2.0 / d1 { let t = t - 1.5 / d1; n1 * t * t + 0.75 }
                else if t < 2.5 / d1 { let t = t - 2.25 / d1; n1 * t * t + 0.9375 }
                else { let t = t - 2.625 / d1; n1 * t * t + 0.984375 }
            }
            Easing::CubicBezier(x1, y1, x2, y2) => {
                // 用牛顿法解三次贝塞尔 x(t) 的参数 t
                if t <= 0.0 { return 0.0; }
                if t >= 1.0 { return 1.0; }
                // 二分查找近似
                let mut low = 0.0;
                let mut high = 1.0;
                for _ in 0..20 {
                    let mid = (low + high) * 0.5;
                    let x = 3.0 * (1.0 - mid) * (1.0 - mid) * mid * x1
                        + 3.0 * (1.0 - mid) * mid * mid * x2
                        + mid * mid * mid;
                    if (x - t).abs() < 0.0001 { break; }
                    if x < t { low = mid; } else { high = mid; }
                }
                let guess = (low + high) * 0.5;
                // 计算 y(guess)
                3.0 * (1.0 - guess) * (1.0 - guess) * guess * y1
                    + 3.0 * (1.0 - guess) * guess * guess * y2
                    + guess * guess * guess
            }
        }
    }
}

impl Default for Easing {
    fn default() -> Self { Self::Linear }
}

/// 单次动画实例。
#[derive(Debug, Clone)]
pub struct Animation {
    /// 动画起始时间。
    start: Instant,
    /// 动画持续时间（秒）。
    duration: f32,
    /// 缓动函数。
    easing: Easing,
    /// 是否循环。
    repeat: bool,
    /// 已完成的循环次数。
    completed_cycles: u32,
}

impl Animation {
    /// 创建新动画。
    pub fn new(duration: f32, easing: Easing) -> Self {
        Self {
            start: Instant::now(),
            duration: duration.max(0.001),
            easing,
            repeat: false,
            completed_cycles: 0,
        }
    }

    /// 设置为循环播放。
    pub fn repeating(mut self) -> Self {
        self.repeat = true;
        self.completed_cycles = 0;
        self
    }

    /// 重新开始动画。
    pub fn restart(&mut self) {
        self.start = Instant::now();
        self.completed_cycles = 0;
    }

    /// 获取当前进度 (0~1)。
    pub fn progress(&self) -> f32 {
        let elapsed = self.start.elapsed().as_secs_f32();
        if self.repeat {
            let t = (elapsed / self.duration).fract();
            self.easing.apply(t)
        } else {
            let t = (elapsed / self.duration).min(1.0);
            self.easing.apply(t)
        }
    }

    /// 动画是否已完成（非循环模式）。
    pub fn is_finished(&self) -> bool {
        !self.repeat && self.start.elapsed().as_secs_f32() >= self.duration
    }
}

/// 数值插值器。
pub trait Interpolatable: Copy + Clone {
    fn lerp(self, other: Self, t: f32) -> Self;
}

impl Interpolatable for f32 {
    fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * t
    }
}

impl Interpolatable for f64 {
    fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * t as f64
    }
}

/// 在起始值和目标值之间驱动动画。
#[derive(Debug, Clone)]
pub struct Tween<T: Interpolatable> {
    pub from: T,
    pub to: T,
    pub animation: Animation,
}

impl<T: Interpolatable> Tween<T> {
    pub fn new(from: T, to: T, duration: f32, easing: Easing) -> Self {
        Self {
            from,
            to,
            animation: Animation::new(duration, easing),
        }
    }

    /// 获取当前插值后的值。
    pub fn value(&self) -> T {
        let t = self.animation.progress();
        self.from.lerp(self.to, t)
    }

    /// 动画是否完成。
    pub fn is_finished(&self) -> bool {
        self.animation.is_finished()
    }

    /// 重新开始。
    pub fn restart(&mut self) {
        self.animation.restart();
    }

    /// 设置循环。
    pub fn repeating(mut self) -> Self {
        self.animation.repeat = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_bounds() {
        assert!((Easing::Linear.apply(0.0) - 0.0).abs() < 0.001);
        assert!((Easing::Linear.apply(1.0) - 1.0).abs() < 0.001);
        assert!((Easing::EaseIn.apply(0.5) - 0.125).abs() < 0.001);
    }

    #[test]
    fn test_tween_f32() {
        let tween = Tween::new(0.0, 100.0, 1.0, Easing::Linear);
        // 刚创建时 progress ≈ 0，value ≈ 0
        assert!((tween.value() - 0.0_f32).abs() < 10.0);
    }
}
