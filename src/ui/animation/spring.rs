//! 基于阻尼谐振子解析解的弹簧动画。

use std::fmt;
use std::sync::{Arc, Mutex};

use crate::ui::traits::Animatable;

type FinishCallback = Arc<Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>>;

const DEFAULT_STIFFNESS: f64 = 170.0;
const DEFAULT_DAMPING: f64 = 20.0;
const DEFAULT_MASS: f64 = 1.0;
const DEFAULT_REST_SPEED: f64 = 0.001;
const DEFAULT_REST_DISPLACEMENT: f64 = 0.001;
const CRITICAL_EPSILON: f64 = 1e-6;

/// 弹簧物理参数。
///
/// `velocity` 使用“整段起止距离 / 秒”为单位；正值朝向目标，负值背离目标。
/// 非有限或非正的刚度、阻尼、质量与收敛阈值会保留原配置值。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    stiffness: f64,
    damping: f64,
    mass: f64,
    velocity: f64,
    rest_speed: f64,
    rest_displacement: f64,
}

impl Spring {
    /// 默认物理参数（stiffness 170、damping 20、mass 1）。
    pub const fn custom() -> Self {
        Self {
            stiffness: DEFAULT_STIFFNESS,
            damping: DEFAULT_DAMPING,
            mass: DEFAULT_MASS,
            velocity: 0.0,
            rest_speed: DEFAULT_REST_SPEED,
            rest_displacement: DEFAULT_REST_DISPLACEMENT,
        }
    }

    /// 过冲更明显的弹簧。
    pub const fn bouncy() -> Self {
        Self {
            damping: 12.0,
            ..Self::custom()
        }
    }

    /// 柔和、较慢的弹簧。
    pub const fn gentle() -> Self {
        Self {
            stiffness: 120.0,
            damping: 18.0,
            ..Self::custom()
        }
    }

    /// 快速收敛的弹簧。
    pub const fn snappy() -> Self {
        Self {
            stiffness: 300.0,
            damping: 25.0,
            ..Self::custom()
        }
    }

    /// 默认刚度与质量下的临界阻尼弹簧，不产生过冲。
    pub fn critical() -> Self {
        Self {
            damping: 2.0 * (DEFAULT_STIFFNESS * DEFAULT_MASS).sqrt(),
            ..Self::custom()
        }
    }

    pub fn stiffness(mut self, stiffness: f64) -> Self {
        if stiffness.is_finite() && stiffness > 0.0 {
            self.stiffness = stiffness;
        }
        self
    }

    pub fn damping(mut self, damping: f64) -> Self {
        if damping.is_finite() && damping > 0.0 {
            self.damping = damping;
        }
        self
    }

    pub fn mass(mut self, mass: f64) -> Self {
        if mass.is_finite() && mass > 0.0 {
            self.mass = mass;
        }
        self
    }

    pub fn velocity(mut self, velocity: f64) -> Self {
        if velocity.is_finite() {
            self.velocity = velocity;
        }
        self
    }

    /// 设置实际值空间中的静止速度阈值。
    pub fn rest_speed(mut self, rest_speed: f64) -> Self {
        if rest_speed.is_finite() && rest_speed > 0.0 {
            self.rest_speed = rest_speed;
        }
        self
    }

    /// 设置实际值空间中距目标的静止距离阈值。
    pub fn rest_displacement(mut self, rest_displacement: f64) -> Self {
        if rest_displacement.is_finite() && rest_displacement > 0.0 {
            self.rest_displacement = rest_displacement;
        }
        self
    }

    pub const fn stiffness_value(self) -> f64 {
        self.stiffness
    }

    pub const fn damping_value(self) -> f64 {
        self.damping
    }

    pub const fn mass_value(self) -> f64 {
        self.mass
    }

    pub const fn initial_velocity(self) -> f64 {
        self.velocity
    }

    pub const fn rest_speed_value(self) -> f64 {
        self.rest_speed
    }

    pub const fn rest_displacement_value(self) -> f64 {
        self.rest_displacement
    }

    fn sample(self, elapsed: f64) -> (f64, f64) {
        let omega = (self.stiffness / self.mass).sqrt();
        let damping_ratio = self.damping / (2.0 * (self.stiffness * self.mass).sqrt());
        let displacement = -1.0;
        let velocity = self.velocity;

        let (displacement, velocity) = if damping_ratio < 1.0 - CRITICAL_EPSILON {
            let decay = damping_ratio * omega;
            let damped = omega * (1.0 - damping_ratio * damping_ratio).sqrt();
            let coefficient = (velocity + decay * displacement) / damped;
            let envelope = (-decay * elapsed).exp();
            let cosine = (damped * elapsed).cos();
            let sine = (damped * elapsed).sin();
            let position = envelope * (displacement * cosine + coefficient * sine);
            let speed = envelope
                * ((-decay * displacement + damped * coefficient) * cosine
                    + (-decay * coefficient - damped * displacement) * sine);
            (position, speed)
        } else if damping_ratio > 1.0 + CRITICAL_EPSILON {
            let root = (damping_ratio * damping_ratio - 1.0).sqrt();
            let slow = -omega * (damping_ratio - root);
            let fast = -omega * (damping_ratio + root);
            let slow_coefficient = (velocity - fast * displacement) / (slow - fast);
            let fast_coefficient = displacement - slow_coefficient;
            let slow_term = slow_coefficient * (slow * elapsed).exp();
            let fast_term = fast_coefficient * (fast * elapsed).exp();
            (slow_term + fast_term, slow * slow_term + fast * fast_term)
        } else {
            let coefficient = velocity + omega * displacement;
            let envelope = (-omega * elapsed).exp();
            (
                (displacement + coefficient * elapsed) * envelope,
                (velocity - omega * coefficient * elapsed) * envelope,
            )
        };

        let position = 1.0 + displacement;
        if position.is_finite() && velocity.is_finite() {
            (position, velocity)
        } else {
            (1.0, 0.0)
        }
    }
}

impl Default for Spring {
    fn default() -> Self {
        Self::custom()
    }
}

/// 从 `from` 自然衰减到 `to` 的单次弹簧动画。
#[derive(Clone)]
pub struct SpringAnimation<T: Animatable> {
    pub from: T,
    pub to: T,
    spring: Spring,
    elapsed: f64,
    position: f64,
    velocity: f64,
    running: bool,
    finished: bool,
    finish_callback: Option<FinishCallback>,
}

impl<T: Animatable> SpringAnimation<T> {
    pub fn new(from: T, to: T, spring: Spring) -> Self {
        let finished = T::delta(from, to).abs() <= spring.rest_displacement;
        Self {
            from,
            to,
            spring,
            elapsed: 0.0,
            position: if finished { 1.0 } else { 0.0 },
            velocity: if finished { 0.0 } else { spring.velocity },
            running: !finished,
            finished,
            finish_callback: None,
        }
    }

    /// 设置一次性完成回调；克隆体共享该回调且全局至多触发一次。
    pub fn on_finish<F>(mut self, callback: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        self.finish_callback = Some(Arc::new(Mutex::new(Some(Box::new(callback)))));
        self
    }

    /// 推进 `dt` 秒，并返回当前值。非有限或负 `dt` 按零处理。
    pub fn update(&mut self, dt: f64) -> T {
        if !self.running {
            return self.value();
        }
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.elapsed += dt;
        (self.position, self.velocity) = self.spring.sample(self.elapsed);

        let value = self.value();
        let total_distance = T::delta(self.from, self.to).abs();
        let displacement = T::delta(value, self.to).abs();
        let speed = self.velocity.abs() * total_distance;
        if displacement <= self.spring.rest_displacement && speed <= self.spring.rest_speed {
            self.position = 1.0;
            self.velocity = 0.0;
            self.running = false;
            self.finished = true;
            self.fire_finish_callback();
        }
        self.value()
    }

    pub fn value(&self) -> T {
        T::lerp(self.from, self.to, self.position)
    }

    /// 返回裁剪到 `[0, 1]` 的归一化位置；实际值仍可产生物理过冲。
    pub fn progress(&self) -> f64 {
        self.position.clamp(0.0, 1.0)
    }

    pub const fn velocity(&self) -> f64 {
        self.velocity
    }

    pub const fn is_running(&self) -> bool {
        self.running
    }

    pub const fn is_finished(&self) -> bool {
        self.finished
    }

    pub const fn spring(&self) -> Spring {
        self.spring
    }

    pub fn pause(&mut self) {
        self.running = false;
    }

    pub fn resume(&mut self) {
        if !self.finished {
            self.running = true;
        }
    }

    /// 跳到目标并停止，不触发完成回调。
    pub fn stop(&mut self) {
        self.position = 1.0;
        self.velocity = 0.0;
        self.running = false;
        self.finished = true;
    }

    pub fn restart(&mut self) {
        self.elapsed = 0.0;
        let finished = T::delta(self.from, self.to).abs() <= self.spring.rest_displacement;
        self.position = if finished { 1.0 } else { 0.0 };
        self.velocity = if finished { 0.0 } else { self.spring.velocity };
        self.running = !finished;
        self.finished = finished;
    }

    pub fn reverse(&mut self) {
        std::mem::swap(&mut self.from, &mut self.to);
        self.restart();
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

impl<T> fmt::Debug for SpringAnimation<T>
where
    T: Animatable + fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SpringAnimation")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("spring", &self.spring)
            .field("elapsed", &self.elapsed)
            .field("position", &self.position)
            .field("velocity", &self.velocity)
            .field("running", &self.running)
            .field("finished", &self.finished)
            .field("has_finish_callback", &self.finish_callback.is_some())
            .finish()
    }
}
