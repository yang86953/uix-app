//! Animation — 通用动画实例，支持任意 `Animatable` 类型。

use crate::ui::animation::easing::{Animatable, Easing};

// ════════════════════════════════════════════════════════════════════════════
// Animation<T>
// ════════════════════════════════════════════════════════════════════════════

/// 一个动画实例，从 `from` 驱动到 `to`，使用缓动曲线 `easing`，持续 `duration` 秒。
#[derive(Debug, Clone)]
pub struct Animation<T: Animatable> {
    /// 起始值。
    pub from: T,
    /// 目标值。
    pub to: T,
    /// 持续时间（秒）。
    pub duration: f64,
    /// 已运行时间（秒）。
    pub elapsed: f64,
    /// 缓动函数。
    pub easing: Easing,
    /// 是否正在运行（暂停时设为 false）。
    pub running: bool,
}

impl<T: Animatable> Animation<T> {
    /// 创建一个新的从 `from` 到 `to` 的动画，持续 `duration` 秒。
    pub fn new(from: T, to: T, duration: f64) -> Self {
        Self {
            from,
            to,
            duration,
            elapsed: 0.0,
            easing: Easing::antd_default(),
            running: true,
        }
    }

    /// 设置缓动函数。
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// 设置初始已用时间（用于从中间状态开始）。
    pub fn with_elapsed(mut self, elapsed: f64) -> Self {
        self.elapsed = elapsed.clamp(0.0, self.duration);
        self
    }

    /// 推进 `dt` 秒。返回当前插值后的值。
    pub fn update(&mut self, dt: f64) -> T {
        if self.running {
            self.elapsed = (self.elapsed + dt).min(self.duration);
        }
        self.current_value()
    }

    /// 获取当前插值后的值（不推进时间）。
    pub fn current_value(&self) -> T {
        let progress = if self.duration > 0.0 {
            (self.elapsed / self.duration).min(1.0)
        } else {
            1.0
        };
        let eased = self.easing.sample(progress);
        T::lerp(self.from, self.to, eased)
    }

    /// 动画是否已完成（elapsed >= duration）。
    pub fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }

    /// 已完成进度的比例 [0, 1]。
    pub fn progress(&self) -> f64 {
        if self.duration > 0.0 {
            (self.elapsed / self.duration).min(1.0)
        } else {
            1.0
        }
    }

    /// 反转动画（交换 from/to 并重置 elapsed）。
    pub fn reverse(&mut self) {
        std::mem::swap(&mut self.from, &mut self.to);
        self.elapsed = 0.0;
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 便捷构造函数
// ════════════════════════════════════════════════════════════════════════════

impl Animation<f32> {
    /// 创建一个从不透明度 0→1 的渐入动画。
    pub fn fade_in(duration: f64) -> Self {
        Self::new(0.0, 1.0, duration).with_easing(Easing::antd_default())
    }

    /// 创建一个从不透明度 1→0 的渐出动画。
    pub fn fade_out(duration: f64) -> Self {
        Self::new(1.0, 0.0, duration).with_easing(Easing::antd_default())
    }
}

impl Animation<f64> {
    /// 从不透明度 0→1 的渐入动画。
    pub fn fade_in(duration: f64) -> Self {
        Self::new(0.0, 1.0, duration).with_easing(Easing::antd_default())
    }

    /// 从不透明度 1→0 的渐出动画。
    pub fn fade_out(duration: f64) -> Self {
        Self::new(1.0, 0.0, duration).with_easing(Easing::antd_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::Color;

    #[test]
    fn animation_linear_progress() {
        let mut anim = Animation::new(0.0f32, 100.0, 1.0).with_easing(Easing::Linear);
        assert!((anim.current_value() - 0.0).abs() < 1e-6);
        anim.update(0.5);
        assert!((anim.current_value() - 50.0).abs() < 0.1);
        anim.update(0.5);
        assert!((anim.current_value() - 100.0).abs() < 0.1);
        assert!(anim.is_finished());
    }

    #[test]
    fn animation_color_transition() {
        let red = Color::from_rgb(255, 0, 0);
        let blue = Color::from_rgb(0, 0, 255);
        let mut anim = Animation::new(red, blue, 1.0).with_easing(Easing::Linear);
        let mid = anim.update(0.5);
        // 线性插值：红(128,0,128) 在 RGB 空间
        assert_eq!(mid.r, 128);
        assert_eq!(mid.b, 128);
    }

    #[test]
    fn animation_custom_easing() {
        let anim =
            Animation::new(0.0f64, 1.0, 1.0).with_easing(Easing::CubicBezier(0.42, 0.0, 0.58, 1.0));
        // 0 进度时为 0
        assert!((anim.current_value() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn animation_reverse() {
        let mut anim = Animation::new(0.0f32, 100.0, 1.0);
        anim.update(0.3);
        anim.reverse();
        assert!((anim.from - 100.0).abs() < 1e-6);
        assert!((anim.to - 0.0).abs() < 1e-6);
        assert!((anim.elapsed - 0.0).abs() < 1e-6);
    }
}
