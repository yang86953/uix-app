//! Declarative, runtime-driven animation values.

use super::{
    Animation, AnimationGroupItem, Easing, Keyframe, KeyframeAnimation, KeyframeError,
    KeyframePlayback, Spring, SpringAnimation, group::GroupItemTiming,
};

mod playback;

use self::playback::{AnimatedMotion, AnimatedPlayback, LoopMode, normalized_delay};
use crate::core::WidgetId;
use crate::ui::animation::traits::Animatable;
use crate::ui::reactive::state::State;
use std::cell::RefCell;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// 减少动效全局开关：开启后过渡重定向直接落到目标值，不申请动画帧。
static REDUCED_MOTION: AtomicBool = AtomicBool::new(false);

/// 返回减少动效是否已开启。
pub fn reduced_motion() -> bool {
    REDUCED_MOTION.load(Ordering::Acquire)
}

/// 设置进程级减少动效开关。
///
/// 范围（诚实边界）：只影响声明式定时 transition 的重定向——开启后的
/// 新过渡与仍在进行中的同目标过渡会立即落到目标值并释放定时帧；进行中
/// 的其他播放（spring、keyframes、进出场 `enter`/`leave`）不被强制跳帧，
/// 照常自然完成。本开关不自动检测 OS 无障碍偏好，由宿主应用显式设置。
pub fn set_reduced_motion(reduced: bool) {
    REDUCED_MOTION.store(reduced, Ordering::Release);
}

static NEXT_ANIMATED_ID: AtomicUsize = AtomicUsize::new(1);

thread_local! {
    static ANIMATED_CAPTURE_STACK: RefCell<Vec<Vec<Arc<dyn AnimatedSource>>>> =
        const { RefCell::new(Vec::new()) };
}

pub(crate) trait AnimatedSource: Send + Sync {
    fn work_id(&self) -> WidgetId;
    fn bind_owner(&self, tree_scope: u64) -> bool;
    fn unbind_owner(&self, tree_scope: u64);
    fn registration(&self) -> AnimatedRegistration;
    fn advance(&self, now: Instant, dt: f64) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnimatedRegistration {
    Inactive,
    Open,
    Deadline(Instant),
}

pub(crate) fn begin_animated_capture() {
    ANIMATED_CAPTURE_STACK.with(|stack| stack.borrow_mut().push(Vec::new()));
}

pub(crate) fn end_animated_capture() -> Vec<Arc<dyn AnimatedSource>> {
    ANIMATED_CAPTURE_STACK.with(|stack| stack.borrow_mut().pop().unwrap_or_default())
}

fn capture_animated_source(source: Arc<dyn AnimatedSource>) {
    ANIMATED_CAPTURE_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        let Some(captured) = stack.last_mut() else {
            return;
        };
        let work_id = source.work_id();
        if captured
            .iter()
            .any(|existing| existing.work_id() == work_id)
        {
            return;
        }
        captured.push(source);
    });
}

struct AnimatedInner<T: Animatable + Sync> {
    work_id: WidgetId,
    current: State<T>,
    playback: Mutex<Option<AnimatedPlayback<T>>>,
    owner_tree_scope: Mutex<Option<u64>>,
}

impl<T: Animatable + Sync> AnimatedInner<T> {
    fn touch(&self) {
        self.current.set(self.current.get_untracked());
    }
}

impl<T: Animatable + Sync> AnimatedSource for AnimatedInner<T> {
    fn work_id(&self) -> WidgetId {
        self.work_id
    }

    fn bind_owner(&self, tree_scope: u64) -> bool {
        let mut owner = self
            .owner_tree_scope
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match *owner {
            Some(current) => current == tree_scope,
            None => {
                *owner = Some(tree_scope);
                true
            }
        }
    }

    fn unbind_owner(&self, tree_scope: u64) {
        let mut owner = self
            .owner_tree_scope
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *owner == Some(tree_scope) {
            *owner = None;
        }
    }

    fn registration(&self) -> AnimatedRegistration {
        self.playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map_or(AnimatedRegistration::Inactive, |playback| {
                playback.registration()
            })
    }

    fn advance(&self, now: Instant, dt: f64) -> bool {
        let (value, active, callback) = {
            let mut playback = self
                .playback
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(playback) = playback.as_mut() else {
                return false;
            };
            let (value, active) = playback.advance(now, dt);
            let callback = playback.take_finished_callback();
            (value, active, callback)
        };
        if let Some(value) = value {
            self.current.set(value);
        }
        if let Some(callback) = callback {
            callback();
        }
        active
    }
}

/// A timed or spring animation value advanced by the owning window's frame loop.
///
/// Reading [`value`](Self::value) while building `App::root` registers the value
/// with that window. Clones share one transition and one current value.
#[derive(Clone)]
pub struct Animated<T: Animatable + Sync> {
    inner: Arc<AnimatedInner<T>>,
}

impl<T: Animatable + Sync> Animated<T> {
    /// Creates a resting animation value.
    pub fn new(initial: T) -> Self {
        Self {
            inner: Arc::new(AnimatedInner {
                work_id: WidgetId::new(NEXT_ANIMATED_ID.fetch_add(1, Ordering::Relaxed)),
                current: State::new(initial),
                playback: Mutex::new(None),
                owner_tree_scope: Mutex::new(None),
            }),
        }
    }

    /// Starts a transition and returns the same shared handle for builder-style use.
    pub fn to(self, target: T, duration: f64, easing: Easing) -> Self {
        self.animate_to(target, duration, easing);
        self
    }

    /// 在 `delay` 秒后启动过渡，并返回同一共享句柄。
    pub fn to_after(self, delay: f64, target: T, duration: f64, easing: Easing) -> Self {
        self.animate_to_after(delay, target, duration, easing);
        self
    }

    /// 使用弹簧物理参数启动单次过渡，并返回同一共享句柄。
    pub fn to_spring(self, target: T, spring: Spring) -> Self {
        self.animate_to_spring(target, spring);
        self
    }

    /// 启动单次 typed keyframe 序列，并返回同一共享句柄。
    pub fn to_keyframes(
        self,
        frames: impl IntoIterator<Item = Keyframe<T>>,
        duration: f64,
    ) -> Result<Self, KeyframeError> {
        self.animate_keyframes_with(frames, KeyframePlayback::new(duration))?;
        Ok(self)
    }

    /// 使用完整播放配置启动 typed keyframe 序列，并返回同一共享句柄。
    pub fn to_keyframes_with(
        self,
        frames: impl IntoIterator<Item = Keyframe<T>>,
        playback: KeyframePlayback,
    ) -> Result<Self, KeyframeError> {
        // 委托共享原位配置入口。
        self.animate_keyframes_with(frames, playback)?;
        // 返回同一共享句柄供链式绑定。
        Ok(self)
    }

    /// Erases the value type so this source can be scheduled by `AnimationGroup`.
    pub fn group_item(&self) -> AnimationGroupItem {
        self.into()
    }

    /// Attaches a one-shot callback to the current transition.
    ///
    /// Clones share the callback. Replacing the playback discards it, while
    /// `stop()` leaves it armed for a later `restart()`. A transition already
    /// at its terminal value invokes the callback before this method returns.
    pub fn on_finish<F>(self, callback: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        self.set_on_finish(callback);
        self
    }

    /// Replaces the one-shot callback attached to the current transition.
    pub fn set_on_finish<F>(&self, callback: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let callback = Box::new(callback) as Box<dyn FnOnce() + Send + 'static>;
        let callback = {
            let mut playback = self
                .inner
                .playback
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            match playback.as_mut() {
                Some(playback) => playback.replace_finish_callback(callback),
                None => Some(callback),
            }
        };
        if let Some(callback) = callback {
            callback();
        }
    }

    pub(crate) fn group_source_id(&self) -> WidgetId {
        self.inner.work_id
    }

    pub(crate) fn group_progress(&self) -> f64 {
        capture_animated_source(self.inner.clone());
        let _ = self.inner.current.get();
        self.inner
            .playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map_or(1.0, AnimatedPlayback::group_progress)
    }

    /// Retargets the shared value from its current position.
    pub fn animate_to(&self, target: T, duration: f64, easing: Easing) {
        self.replace_playback(target, duration, easing, Duration::ZERO);
    }

    /// 在 `delay` 秒后从当前值开始过渡；等待期只登记 deadline，不申请动画帧。
    pub fn animate_to_after(&self, delay: f64, target: T, duration: f64, easing: Easing) {
        self.replace_playback(target, duration, easing, normalized_delay(delay));
    }

    /// 从当前值重新定向到一个单次弹簧过渡。
    pub fn animate_to_spring(&self, target: T, spring: Spring) {
        let from = self.inner.current.get_untracked();
        let next = SpringAnimation::new(from, target, spring);
        let finished = next.is_finished();
        let value = next.value();
        *self
            .inner
            .playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Some(AnimatedPlayback::new_spring(next));
        if finished {
            self.inner.current.set(value);
        } else {
            self.inner.touch();
        }
    }

    /// 从 keyframe 序列首值启动单次过渡；缺少 offset 0 时从当前值补齐。
    pub fn animate_keyframes(
        &self,
        frames: impl IntoIterator<Item = Keyframe<T>>,
        duration: f64,
    ) -> Result<(), KeyframeError> {
        // 使用兼容旧 API 的单次正放配置。
        self.animate_keyframes_with(frames, KeyframePlayback::new(duration))
    }

    /// 使用完整播放配置从当前值启动 typed keyframe 序列。
    pub fn animate_keyframes_with(
        &self,
        frames: impl IntoIterator<Item = Keyframe<T>>,
        playback: KeyframePlayback,
    ) -> Result<(), KeyframeError> {
        // 保存关键帧外部基础值供 fill-mode 恢复。
        let current = self.inner.current.get_untracked();
        // 使用声明时长构造规范化关键帧序列。
        let next = KeyframeAnimation::from_current(current, frames, playback.duration)?;
        // 使用统一播放状态机组合延迟、迭代、方向与填充。
        let next = AnimatedPlayback::new_keyframes(next, current, playback, Instant::now());
        // 延迟期可见值由填充策略决定。
        let value = next.value();
        // 原子替换当前播放状态。
        *self
            .inner
            .playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(next);
        // 发布初始可见值并触发声明式依赖。
        self.inner.current.set(value);
        // 返回成功。
        Ok(())
    }

    fn replace_playback(&self, target: T, duration: f64, easing: Easing, delay: Duration) {
        let from = self.inner.current.get_untracked();
        let mut next = Animation::new(from, target, duration).easing(easing);
        // 减少动效与零时长同路：立即落到目标值，不登记帧截止。
        let immediate = reduced_motion() || (delay.is_zero() && next.duration <= 0.0);
        if immediate {
            next.stop();
        }
        let value = next.value();
        let mut playback = self
            .inner
            .playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let loop_mode = playback
            .as_ref()
            .map_or(LoopMode::Once, |playback| playback.loop_mode);
        *playback = Some(AnimatedPlayback::new(
            next,
            loop_mode,
            delay,
            Instant::now(),
        ));
        drop(playback);
        if immediate {
            self.inner.current.set(value);
        } else {
            self.inner.touch();
        }
    }

    /// Repeats a fixed-duration transition indefinitely; Spring remains single-play.
    pub fn loop_forever(self) -> Self {
        self.configure_loop(LoopMode::Forever);
        self
    }

    /// Alternates a fixed-duration transition indefinitely; Spring remains single-play.
    pub fn loop_alternate(self) -> Self {
        self.configure_loop(LoopMode::Alternate);
        self
    }

    /// Plays a fixed-duration transition `count` times in total; Spring remains single-play.
    ///
    /// A count of zero commits the target immediately without frame work.
    pub fn loop_count(self, count: u64) -> Self {
        self.configure_loop(LoopMode::Count(count));
        self
    }

    /// Reads the current value and registers it with the active root build.
    pub fn value(&self) -> T {
        capture_animated_source(self.inner.clone());
        self.inner.current.get()
    }

    /// Returns transition progress in `[0, 1]` and registers the root dependency.
    pub fn progress(&self) -> f64 {
        capture_animated_source(self.inner.clone());
        let _ = self.inner.current.get();
        self.inner
            .playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map_or(1.0, AnimatedPlayback::progress)
    }

    /// Returns whether the current transition is at rest.
    pub fn is_finished(&self) -> bool {
        capture_animated_source(self.inner.clone());
        let _ = self.inner.current.get();
        self.inner
            .playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .is_none_or(AnimatedPlayback::is_finished)
    }

    /// Pauses at the current value and stops requesting animation frames.
    pub fn pause(&self) {
        self.group_pause_at(Instant::now());
    }

    pub(crate) fn group_pause_at(&self, now: Instant) {
        if let Some(playback) = self
            .inner
            .playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_mut()
        {
            playback.pause(now);
        }
        self.inner.touch();
    }

    /// Resumes a paused, unfinished transition.
    pub fn resume(&self) {
        self.group_resume_at(Instant::now());
    }

    pub(crate) fn group_resume_at(&self, now: Instant) {
        if let Some(playback) = self
            .inner
            .playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_mut()
        {
            playback.resume(now);
        }
        self.inner.touch();
    }

    /// Jumps to the target and stops requesting frames.
    pub fn stop(&self) {
        let value = {
            let mut playback = self
                .inner
                .playback
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(playback) = playback.as_mut() else {
                return;
            };
            playback.stop();
            playback.value()
        };
        self.inner.current.set(value);
    }

    /// Restarts the current transition from its original start value.
    pub fn restart(&self) {
        self.group_restart_at(Instant::now());
    }

    pub(crate) fn group_restart_at(&self, now: Instant) {
        let value = {
            let mut playback = self
                .inner
                .playback
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(playback) = playback.as_mut() else {
                return;
            };
            playback.restart(now);
            playback.value()
        };
        self.inner.current.set(value);
    }

    pub(crate) fn group_timing(&self) -> GroupItemTiming {
        let playback = self
            .inner
            .playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(playback) = playback.as_ref() else {
            return GroupItemTiming::Inactive;
        };
        if !playback.delay.is_zero() {
            return GroupItemTiming::Delayed;
        }

        let seconds = match (&playback.motion, playback.loop_mode) {
            (AnimatedMotion::Timed(animation), LoopMode::Once) => animation.duration,
            (AnimatedMotion::Timed(animation), LoopMode::Count(count)) => {
                animation.duration * count as f64
            }
            (AnimatedMotion::Timed(animation), LoopMode::AlternateCount(count)) => {
                // 有限交替定时动画与普通计数动画具有相同总时长。
                animation.duration * count as f64
            }
            (AnimatedMotion::Timed(_), LoopMode::Forever | LoopMode::Alternate) => {
                return GroupItemTiming::Unbounded;
            }
            (AnimatedMotion::Keyframes(animation), LoopMode::Once) => animation.duration(),
            (AnimatedMotion::Keyframes(animation), LoopMode::Count(count))
            | (AnimatedMotion::Keyframes(animation), LoopMode::AlternateCount(count)) => {
                // 有限关键帧轮次按单轮时长求总时长。
                animation.duration() * count as f64
            }
            (AnimatedMotion::Keyframes(_), LoopMode::Forever | LoopMode::Alternate) => {
                // 无限关键帧序列不能形成有限动画组时长。
                return GroupItemTiming::Unbounded;
            }
            (AnimatedMotion::Spring(_), _) => return GroupItemTiming::Unbounded,
        };
        Duration::try_from_secs_f64(seconds)
            .map(GroupItemTiming::Finite)
            .unwrap_or(GroupItemTiming::Unbounded)
    }

    pub(crate) fn group_schedule_at(&self, delay: Duration, now: Instant) {
        let value = {
            let mut playback = self
                .inner
                .playback
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(playback) = playback.as_mut() else {
                return;
            };
            playback.delay = delay;
            playback.restart(now);
            playback.value()
        };
        self.inner.current.set(value);
    }

    /// Swaps the current transition endpoints and restarts it.
    pub fn reverse(&self) {
        let value = {
            let mut playback = self
                .inner
                .playback
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(playback) = playback.as_mut() else {
                return;
            };
            playback.reverse(Instant::now());
            playback.value()
        };
        self.inner.current.set(value);
    }

    fn configure_loop(&self, loop_mode: LoopMode) {
        let value = self
            .inner
            .playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_mut()
            .and_then(|playback| playback.configure_loop(loop_mode, Instant::now()));
        if let Some(value) = value {
            self.inner.current.set(value);
        }
    }
}

impl<T> fmt::Debug for Animated<T>
where
    T: Animatable + Sync + fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Animated")
            .field("work_id", &self.inner.work_id)
            .field("value", &self.inner.current.get_untracked())
            .field(
                "playback",
                &self
                    .inner
                    .playback
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
            )
            .finish()
    }
}
