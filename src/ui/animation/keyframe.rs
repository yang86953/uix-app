//! 类型化单值关键帧动画。

use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::ui::animation::traits::Animatable;

use super::Easing;

type FinishCallback = Arc<Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>>;

/// 声明关键帧序列每轮的播放方向。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyframeDirection {
    /// 每轮都从起点播放到终点。
    #[default]
    Normal,
    /// 每轮都从终点播放到起点。
    Reverse,
    /// 奇数轮正放、偶数轮倒放。
    Alternate,
    /// 奇数轮倒放、偶数轮正放。
    AlternateReverse,
}

/// 声明关键帧在延迟期与完成后的可见填充行为。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyframeFillMode {
    /// 延迟期与完成后都显示调用方基础值。
    #[default]
    None,
    /// 完成后保留最后一个可见关键帧值。
    Forwards,
    /// 延迟期显示第一帧值，完成后恢复基础值。
    Backwards,
    /// 延迟期显示第一帧值且完成后保留最后一帧值。
    Both,
}

/// 配置关键帧播放的完整时间、迭代、方向与填充契约。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyframePlayback {
    /// 单轮动画时长，单位为秒。
    pub duration: f64,
    /// 首轮开始前的等待时长，单位为秒。
    pub delay: f64,
    /// 总播放轮数；`None` 表示无限循环。
    pub iterations: Option<u64>,
    /// 每轮正放、倒放或交替方向。
    pub direction: KeyframeDirection,
    /// 延迟期与完成后的值填充策略。
    pub fill_mode: KeyframeFillMode,
}

// 提供保持旧 typed keyframe 行为的紧凑构造与链式配置。
impl KeyframePlayback {
    /// 创建单次正放并保留终值的关键帧配置。
    pub const fn new(duration: f64) -> Self {
        // 返回兼容既有 `to_keyframes` 的默认行为。
        Self {
            // 保存调用方声明的单轮时长。
            duration,
            // 默认没有启动延迟。
            delay: 0.0,
            // 默认只播放一次。
            iterations: Some(1),
            // 默认正向播放。
            direction: KeyframeDirection::Normal,
            // 旧 API 完成后一直保留终值。
            fill_mode: KeyframeFillMode::Forwards,
        }
    }

    /// 设置首轮开始前的等待时长。
    pub const fn with_delay(mut self, delay: f64) -> Self {
        // 保存原始值并由运行时统一归一化。
        self.delay = delay;
        // 返回更新后的配置。
        self
    }

    /// 设置有限总播放轮数。
    pub const fn with_iterations(mut self, iterations: u64) -> Self {
        // Some 明确区分有限轮数与无限循环。
        self.iterations = Some(iterations);
        // 返回更新后的配置。
        self
    }

    /// 设置为无限循环。
    pub const fn infinite(mut self) -> Self {
        // None 表示调度器需要持续续帧。
        self.iterations = None;
        // 返回更新后的配置。
        self
    }

    /// 设置每轮播放方向。
    pub const fn with_direction(mut self, direction: KeyframeDirection) -> Self {
        // 保存闭合方向枚举。
        self.direction = direction;
        // 返回更新后的配置。
        self
    }

    /// 设置延迟期与完成后的填充行为。
    pub const fn with_fill_mode(mut self, fill_mode: KeyframeFillMode) -> Self {
        // 保存闭合填充枚举。
        self.fill_mode = fill_mode;
        // 返回更新后的配置。
        self
    }
}

/// 在归一化时间线上的一个偏移处声明的值与其缓动曲线。
///
/// 缓动曲线属于从该关键帧开始的片段；因此最后一帧的缓动会被忽略。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe<T> {
    /// 此帧在归一化时间线上的位置。
    pub offset: f64,
    /// 动画到达此帧时对应的值。
    pub value: T,
    /// 从此帧到下一帧的插值缓动曲线。
    pub easing: Easing,
}

impl<T> Keyframe<T> {
    /// 创建使用线性缓动的关键帧。
    pub const fn new(offset: f64, value: T) -> Self {
        Self {
            offset,
            value,
            easing: Easing::Linear,
        }
    }

    /// 设置从此帧开始的片段缓动曲线。
    pub const fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}

/// 关键帧序列构造失败。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyframeError {
    /// 构造序列时没有提供任何关键帧。
    Empty,
    /// 至少一个关键帧偏移不是有限数值。
    NonFiniteOffset,
}

impl fmt::Display for KeyframeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("keyframe sequence must not be empty"),
            Self::NonFiniteOffset => formatter.write_str("keyframe offset must be finite"),
        }
    }
}

impl Error for KeyframeError {}

/// 固定时长、类型化单值关键帧序列。
#[derive(Clone)]
pub struct KeyframeAnimation<T: Animatable> {
    frames: Vec<Keyframe<T>>,
    duration: f64,
    elapsed: f64,
    running: bool,
    reversed: bool,
    finish_callback: Option<FinishCallback>,
}

impl<T: Animatable> KeyframeAnimation<T> {
    /// 构建归一化序列。
    ///
    /// 偏移会被钳制到 `[0, 1]` 再稳定排序。重复偏移处后声明的帧生效。
    /// 缺失的边界用首个或末个已声明值补齐。
    pub fn new(
        frames: impl IntoIterator<Item = Keyframe<T>>,
        duration: f64,
    ) -> Result<Self, KeyframeError> {
        Self::build(frames, duration, None)
    }

    pub(crate) fn from_current(
        current: T,
        frames: impl IntoIterator<Item = Keyframe<T>>,
        duration: f64,
    ) -> Result<Self, KeyframeError> {
        Self::build(frames, duration, Some(current))
    }

    fn build(
        frames: impl IntoIterator<Item = Keyframe<T>>,
        duration: f64,
        current: Option<T>,
    ) -> Result<Self, KeyframeError> {
        // 逐帧校验偏移有限性并钳制到 [0, 1]。
        let mut frames = frames
            .into_iter()
            .map(|mut frame| {
                if !frame.offset.is_finite() {
                    return Err(KeyframeError::NonFiniteOffset);
                }
                frame.offset = frame.offset.clamp(0.0, 1.0);
                if frame.offset == 0.0 {
                    frame.offset = 0.0;
                }
                Ok(frame)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if frames.is_empty() {
            return Err(KeyframeError::Empty);
        }

        // 按偏移稳定排序，重复偏移处以后声明的帧覆盖先前的。
        frames.sort_by(|left, right| left.offset.total_cmp(&right.offset));
        let mut normalized = Vec::<Keyframe<T>>::with_capacity(frames.len() + 2);
        for frame in frames {
            if normalized
                .last()
                .is_some_and(|existing| existing.offset == frame.offset)
            {
                if let Some(existing) = normalized.last_mut() {
                    *existing = frame;
                }
            } else {
                normalized.push(frame);
            }
        }

        // 补齐边界：起点不在 0 处时用当前值（或首帧值）补一帧，终点同理。
        let first = normalized[0];
        if first.offset > 0.0 {
            normalized.insert(0, Keyframe::new(0.0, current.unwrap_or(first.value)));
        }
        let last = normalized[normalized.len() - 1];
        if last.offset < 1.0 {
            normalized.push(Keyframe::new(1.0, last.value));
        }

        // 时长非有限或为负时归零；零时长视为立即完成。
        let duration = if duration.is_finite() {
            duration.max(0.0)
        } else {
            0.0
        };
        let finished = duration <= 0.0;
        Ok(Self {
            frames: normalized,
            duration,
            elapsed: if finished { duration } else { 0.0 },
            running: !finished,
            reversed: false,
            finish_callback: None,
        })
    }

    /// 设置由克隆共享的一次性回调，全局最多触发一次。
    pub fn on_finish<F>(mut self, callback: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        self.finish_callback = Some(Arc::new(Mutex::new(Some(Box::new(callback)))));
        self
    }

    /// 将动画推进给定秒数并返回推进后的采样值。
    pub fn update(&mut self, dt: f64) -> T {
        // 仅运行中推进时间；到达时长后停止并触发一次完成回调。
        if self.running {
            let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
            self.elapsed = (self.elapsed + dt).min(self.duration);
            if self.is_finished() {
                self.running = false;
                self.fire_finish_callback();
            }
        }
        self.value()
    }

    /// 返回当前播放方向和进度对应的采样值。
    pub fn value(&self) -> T {
        // 倒放时从时间线末尾镜像进度。
        let progress = if self.reversed {
            1.0 - self.progress()
        } else {
            self.progress()
        };
        self.sample(progress)
    }

    /// 返回不受播放方向影响的归一化已用时进度。
    pub fn progress(&self) -> f64 {
        if self.duration > 0.0 {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// 返回已用时是否到达动画时长。
    pub fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }

    /// 返回动画当前是否会由 [`Self::update`] 推进。
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// 返回规范化后的动画时长（秒）。
    pub const fn duration(&self) -> f64 {
        self.duration
    }

    // 返回播放状态机计算跨轮推进所需的当前已用时。
    pub(crate) const fn elapsed(&self) -> f64 {
        // 暴露只读时间值但不转移关键帧所有权。
        self.elapsed
    }

    // 把同一关键帧对象定位到指定轮次内时间与方向。
    pub(crate) fn seek(&mut self, elapsed: f64, reversed: bool) {
        // 把非有限输入归零并钳制到单轮时长。
        self.elapsed = if elapsed.is_finite() {
            // 有限时间限制在合法闭区间。
            elapsed.clamp(0.0, self.duration)
        } else {
            // 非有限输入不能污染动画时间线。
            0.0
        };
        // 保存当前轮次的明确方向。
        self.reversed = reversed;
        // 只有尚未到达终点的正时长动画需要继续推进。
        self.running = self.duration > 0.0 && self.elapsed < self.duration;
    }

    /// 返回已排序、去重并补齐边界的关键帧序列。
    pub fn frames(&self) -> &[Keyframe<T>] {
        &self.frames
    }

    /// 暂停时间推进并保留当前进度。
    pub fn pause(&mut self) {
        self.running = false;
    }

    /// 在动画尚未完成时恢复时间推进。
    pub fn resume(&mut self) {
        if !self.is_finished() {
            self.running = true;
        }
    }

    /// 将进度移到终点并停止，且不触发完成回调。
    pub fn stop(&mut self) {
        self.elapsed = self.duration;
        self.running = false;
    }

    /// 将进度重置到起点，并在时长大于零时开始推进。
    pub fn restart(&mut self) {
        self.elapsed = 0.0;
        self.running = self.duration > 0.0;
    }

    /// 切换正放与倒放方向，并从时间线起点重新开始。
    pub fn reverse(&mut self) {
        self.reversed = !self.reversed;
        self.restart();
    }

    fn sample(&self, progress: f64) -> T {
        // 越界进度直接取端点值。
        if progress <= 0.0 {
            return self.frames[0].value;
        }
        if progress >= 1.0 {
            return self.frames[self.frames.len() - 1].value;
        }

        // 二分定位进度所在片段，并在片段内按缓动曲线插值。
        let upper = self
            .frames
            .partition_point(|frame| frame.offset <= progress);
        let start = self.frames[upper.saturating_sub(1)];
        let end = self.frames[upper];
        let local = (progress - start.offset) / (end.offset - start.offset);
        T::lerp(start.value, end.value, start.easing.sample(local))
    }

    fn fire_finish_callback(&mut self) {
        // 取出共享回调（全局只触发一次），毒锁也继续取。
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

impl<T> fmt::Debug for KeyframeAnimation<T>
where
    T: Animatable + fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KeyframeAnimation")
            .field("frames", &self.frames)
            .field("duration", &self.duration)
            .field("elapsed", &self.elapsed)
            .field("running", &self.running)
            .field("reversed", &self.reversed)
            .field("has_finish_callback", &self.finish_callback.is_some())
            .finish()
    }
}
