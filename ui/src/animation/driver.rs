//! AnimationDriver — 动画驱动引擎，管理所有活跃动画实例。

use crate::animation::core::Animation;
use crate::api::Animatable;

/// 动画完成回调。
pub type AnimationCallback<T> = Box<dyn FnOnce(T) + Send + 'static>;

// ════════════════════════════════════════════════════════════════════════════
// 动画条目
// ════════════════════════════════════════════════════════════════════════════

/// 带标识符的动画条目，包含可选的完成回调。
struct AnimationEntry<T: Animatable> {
    id: u64,
    animation: Animation<T>,
    on_complete: Option<AnimationCallback<T>>,
}

// ════════════════════════════════════════════════════════════════════════════
// AnimationDriver
// ════════════════════════════════════════════════════════════════════════════

/// 动画驱动引擎，持有所有类型擦除后的动画实例，按帧推进。
///
/// 每帧调用 `update(dt)` 推进所有活跃动画，完成后移除并通知回调。
#[derive(Default)]
pub struct AnimationDriver {
    f32_animations: Vec<AnimationEntry<f32>>,
    f64_animations: Vec<AnimationEntry<f64>>,
    next_id: u64,
}

impl AnimationDriver {
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加一个 `f32` 动画，返回其 ID。
    pub fn add_f32(
        &mut self,
        anim: Animation<f32>,
        on_complete: Option<AnimationCallback<f32>>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.f32_animations.push(AnimationEntry {
            id,
            animation: anim,
            on_complete,
        });
        id
    }

    /// 添加一个 `f64` 动画，返回其 ID。
    pub fn add_f64(
        &mut self,
        anim: Animation<f64>,
        on_complete: Option<AnimationCallback<f64>>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.f64_animations.push(AnimationEntry {
            id,
            animation: anim,
            on_complete,
        });
        id
    }

    /// 推进所有动画 `dt` 秒。返回本帧完成动画的 (id, final_value) 列表。
    pub fn update(&mut self, dt: f64) -> Vec<(u64, AnimValue)> {
        let mut completed = Vec::new();

        self.tick_f32(dt, &mut completed);
        self.tick_f64(dt, &mut completed);

        completed
    }

    fn tick_f32(&mut self, dt: f64, completed: &mut Vec<(u64, AnimValue)>) {
        let mut remove_indices = Vec::new();
        for (i, entry) in self.f32_animations.iter_mut().enumerate() {
            if !entry.animation.running {
                continue;
            }
            let was_finished = entry.animation.is_finished();
            let _ = entry.animation.update(dt);
            if !was_finished && entry.animation.is_finished() {
                let final_val = entry.animation.current_value();
                if let Some(cb) = entry.on_complete.take() {
                    cb(final_val);
                }
                completed.push((entry.id, AnimValue::F32(final_val)));
                remove_indices.push(i);
            }
        }
        // 逆序移除
        for &i in remove_indices.iter().rev() {
            self.f32_animations.remove(i);
        }
    }

    fn tick_f64(&mut self, dt: f64, completed: &mut Vec<(u64, AnimValue)>) {
        let mut remove_indices = Vec::new();
        for (i, entry) in self.f64_animations.iter_mut().enumerate() {
            if !entry.animation.running {
                continue;
            }
            let was_finished = entry.animation.is_finished();
            let _ = entry.animation.update(dt);
            if !was_finished && entry.animation.is_finished() {
                let final_val = entry.animation.current_value();
                if let Some(cb) = entry.on_complete.take() {
                    cb(final_val);
                }
                completed.push((entry.id, AnimValue::F64(final_val)));
                remove_indices.push(i);
            }
        }
        for &i in remove_indices.iter().rev() {
            self.f64_animations.remove(i);
        }
    }

    /// 是否有任意动画正在运行。
    pub fn is_any_running(&self) -> bool {
        self.f32_animations.iter().any(|e| e.animation.running)
            || self.f64_animations.iter().any(|e| e.animation.running)
    }

    /// 清除所有动画。
    pub fn clear(&mut self) {
        self.f32_animations.clear();
        self.f64_animations.clear();
    }
}

/// 动画完成时的返回值（类型擦除）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimValue {
    F32(f32),
    F64(f64),
}

impl AnimValue {
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Self::F32(v) => Some(*v),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::F64(v) => Some(*v),
            _ => None,
        }
    }
}
