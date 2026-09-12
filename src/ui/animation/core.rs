//! Animation — 通用动画实例，支持任意 `Animatable` 类型。

use crate::ui::animation::Easing;
use crate::ui::animation::traits::Animatable;
use std::fmt;
use std::sync::{Arc, Mutex};

type FinishCallback = Arc<Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>>;

// ════════════════════════════════════════════════════════════════════════════
// Animation<T>
// ════════════════════════════════════════════════════════════════════════════

/// 一个动画实例，从 `from` 驱动到 `to`，使用缓动曲线 `easing`，持续 `duration` 秒。
#[derive(Clone)]
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
    finish_callback: Option<FinishCallback>,
}

impl<T: Animatable> Animation<T> {
    /// 创建一个新的从 `from` 到 `to` 的动画，持续 `duration` 秒。
    pub fn new(from: T, to: T, duration: f64) -> Self {
        Self {
            from,
            to,
            duration: duration.max(0.0),
            elapsed: 0.0,
            easing: Easing::css_ease(),
            running: true,
            finish_callback: None,
        }
    }

    /// 设置缓动函数。
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// 设置一次性完成回调；克隆体共享该回调且全局至多触发一次。
    pub fn on_finish<F>(mut self, callback: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        self.finish_callback = Some(Arc::new(Mutex::new(Some(Box::new(callback)))));
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
            self.elapsed = (self.elapsed + dt.max(0.0)).min(self.duration);
            if self.is_finished() {
                self.fire_finish_callback();
            }
        }
        self.value()
    }

    /// 获取当前插值后的值（不推进时间）。
    pub fn value(&self) -> T {
        let progress = if self.duration > 0.0 {
            (self.elapsed / self.duration).min(1.0)
        } else {
            1.0
        };
        if progress <= 0.0 {
            return self.from;
        }
        if progress >= 1.0 {
            return self.to;
        }
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

    /// 暂停动画。
    pub fn pause(&mut self) {
        self.running = false;
    }

    /// 继续动画。
    pub fn resume(&mut self) {
        self.running = true;
    }

    /// 停止动画并跳到结束位置。
    pub fn stop(&mut self) {
        self.elapsed = self.duration;
        self.running = false;
    }

    /// 重置动画回到起始位置。
    pub fn restart(&mut self) {
        self.elapsed = 0.0;
        self.running = true;
    }

    /// 反转动画（交换 from/to 并重置 elapsed）。
    pub fn reverse(&mut self) {
        std::mem::swap(&mut self.from, &mut self.to);
        self.elapsed = 0.0;
        self.running = true;
    }

    fn fire_finish_callback(&mut self) {
        let Some(callback) = self.finish_callback.as_ref() else {
            return;
        };
        let callback = match callback.lock() {
            Ok(mut callback) => callback.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };
        self.finish_callback = None;
        if let Some(callback) = callback {
            callback();
        }
    }
}

impl<T> fmt::Debug for Animation<T>
where
    T: Animatable + fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Animation")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("duration", &self.duration)
            .field("elapsed", &self.elapsed)
            .field("easing", &self.easing)
            .field("running", &self.running)
            .field("has_finish_callback", &self.finish_callback.is_some())
            .finish()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 便捷构造函数
// ════════════════════════════════════════════════════════════════════════════

impl Animation<f32> {
    /// 创建一个从不透明度 0→1 的渐入动画。
    pub fn fade_in(duration: f64) -> Self {
        Self::new(0.0, 1.0, duration).easing(Easing::css_ease())
    }

    /// 创建一个从不透明度 1→0 的渐出动画。
    pub fn fade_out(duration: f64) -> Self {
        Self::new(1.0, 0.0, duration).easing(Easing::css_ease())
    }
}

impl Animation<f64> {
    /// 从不透明度 0→1 的渐入动画。
    pub fn fade_in(duration: f64) -> Self {
        Self::new(0.0, 1.0, duration).easing(Easing::css_ease())
    }

    /// 从不透明度 1→0 的渐出动画。
    pub fn fade_out(duration: f64) -> Self {
        Self::new(1.0, 0.0, duration).easing(Easing::css_ease())
    }
}
