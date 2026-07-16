//! Declarative, runtime-driven animation values.

use super::{
    group::GroupItemTiming, Animation, AnimationGroupItem, Easing, Keyframe, KeyframeAnimation,
    KeyframeError, Spring, SpringAnimation,
};
use crate::core::ComponentId;
use crate::ui::foundation::state::State;
use crate::ui::traits::Animatable;
use std::cell::RefCell;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static NEXT_ANIMATED_ID: AtomicUsize = AtomicUsize::new(1);

thread_local! {
    static ANIMATED_CAPTURE_STACK: RefCell<Vec<Vec<Arc<dyn AnimatedSource>>>> =
        const { RefCell::new(Vec::new()) };
}

pub(crate) trait AnimatedSource: Send + Sync {
    fn work_id(&self) -> ComponentId;
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
    work_id: ComponentId,
    current: State<T>,
    playback: Mutex<Option<AnimatedPlayback<T>>>,
    owner_tree_scope: Mutex<Option<u64>>,
}

impl<T: Animatable + Sync> AnimatedInner<T> {
    fn touch(&self) {
        self.current.set(self.current.get_untracked());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoopMode {
    Once,
    Count(u64),
    Forever,
    Alternate,
}

#[derive(Clone, Copy, Debug)]
enum DelayState {
    None,
    Waiting(Instant),
    Paused(Duration),
}

#[derive(Debug)]
enum AnimatedMotion<T: Animatable> {
    Timed(Animation<T>),
    Keyframes(KeyframeAnimation<T>),
    Spring(SpringAnimation<T>),
}

impl<T: Animatable> AnimatedMotion<T> {
    fn value(&self) -> T {
        match self {
            Self::Timed(animation) => animation.value(),
            Self::Keyframes(animation) => animation.value(),
            Self::Spring(animation) => animation.value(),
        }
    }

    fn update(&mut self, dt: f64) -> T {
        match self {
            Self::Timed(animation) => animation.update(dt),
            Self::Keyframes(animation) => animation.update(dt),
            Self::Spring(animation) => animation.update(dt),
        }
    }

    fn progress(&self) -> f64 {
        match self {
            Self::Timed(animation) => animation.progress(),
            Self::Keyframes(animation) => animation.progress(),
            Self::Spring(animation) => animation.progress(),
        }
    }

    fn is_running(&self) -> bool {
        match self {
            Self::Timed(animation) => animation.running,
            Self::Keyframes(animation) => animation.is_running(),
            Self::Spring(animation) => animation.is_running(),
        }
    }

    fn is_finished(&self) -> bool {
        match self {
            Self::Timed(animation) => animation.is_finished(),
            Self::Keyframes(animation) => animation.is_finished(),
            Self::Spring(animation) => animation.is_finished(),
        }
    }

    fn pause(&mut self) {
        match self {
            Self::Timed(animation) => animation.pause(),
            Self::Keyframes(animation) => animation.pause(),
            Self::Spring(animation) => animation.pause(),
        }
    }

    fn resume(&mut self) {
        match self {
            Self::Timed(animation) => animation.resume(),
            Self::Keyframes(animation) => animation.resume(),
            Self::Spring(animation) => animation.resume(),
        }
    }

    fn stop(&mut self) {
        match self {
            Self::Timed(animation) => animation.stop(),
            Self::Keyframes(animation) => animation.stop(),
            Self::Spring(animation) => animation.stop(),
        }
    }

    fn restart(&mut self) {
        match self {
            Self::Timed(animation) => animation.restart(),
            Self::Keyframes(animation) => animation.restart(),
            Self::Spring(animation) => animation.restart(),
        }
    }

    fn reverse(&mut self) {
        match self {
            Self::Timed(animation) => animation.reverse(),
            Self::Keyframes(animation) => animation.reverse(),
            Self::Spring(animation) => animation.reverse(),
        }
    }

    fn supports_loops(&self) -> bool {
        matches!(self, Self::Timed(_))
    }
}

#[derive(Debug)]
struct AnimatedPlayback<T: Animatable> {
    motion: AnimatedMotion<T>,
    original_from: T,
    original_to: T,
    loop_mode: LoopMode,
    completed_plays: u64,
    delay: Duration,
    delay_state: DelayState,
}

impl<T: Animatable> AnimatedPlayback<T> {
    fn new(animation: Animation<T>, loop_mode: LoopMode, delay: Duration, now: Instant) -> Self {
        let mut playback = Self {
            original_from: animation.from,
            original_to: animation.to,
            motion: AnimatedMotion::Timed(animation),
            loop_mode,
            completed_plays: 0,
            delay,
            delay_state: DelayState::None,
        };
        playback.arm_delay(now);
        playback
    }

    fn new_spring(animation: SpringAnimation<T>) -> Self {
        Self {
            original_from: animation.from,
            original_to: animation.to,
            motion: AnimatedMotion::Spring(animation),
            loop_mode: LoopMode::Once,
            completed_plays: 0,
            delay: Duration::ZERO,
            delay_state: DelayState::None,
        }
    }

    fn new_keyframes(animation: KeyframeAnimation<T>) -> Self {
        Self {
            original_from: animation.value(),
            original_to: animation.frames()[animation.frames().len() - 1].value,
            motion: AnimatedMotion::Keyframes(animation),
            loop_mode: LoopMode::Once,
            completed_plays: 0,
            delay: Duration::ZERO,
            delay_state: DelayState::None,
        }
    }

    fn is_active(&self) -> bool {
        matches!(self.delay_state, DelayState::None)
            && self.motion.is_running()
            && !matches!(self.loop_mode, LoopMode::Count(0))
            && !self.motion.is_finished()
    }

    fn registration(&self) -> AnimatedRegistration {
        match self.delay_state {
            DelayState::Waiting(deadline) => AnimatedRegistration::Deadline(deadline),
            DelayState::None if self.is_active() => AnimatedRegistration::Open,
            DelayState::None | DelayState::Paused(_) => AnimatedRegistration::Inactive,
        }
    }

    fn is_finished(&self) -> bool {
        matches!(self.delay_state, DelayState::None) && self.motion.is_finished()
    }

    fn progress(&self) -> f64 {
        match self.delay_state {
            DelayState::None => self.motion.progress(),
            DelayState::Waiting(_) | DelayState::Paused(_) => 0.0,
        }
    }

    fn advance(&mut self, now: Instant, dt: f64) -> (Option<T>, bool) {
        let mut dt = dt;
        match self.delay_state {
            DelayState::Waiting(deadline) if deadline > now => return (None, false),
            DelayState::Waiting(deadline) => {
                self.delay_state = DelayState::None;
                self.motion.resume();
                if self.motion.is_finished() {
                    self.finish_forward(self.completed_plays);
                    return (Some(self.motion.value()), false);
                }
                dt = now.saturating_duration_since(deadline).as_secs_f64();
            }
            DelayState::Paused(_) => return (None, false),
            DelayState::None => {}
        }
        if !self.is_active() {
            return (None, false);
        }
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        if dt <= 0.0 {
            return (None, true);
        }
        let (value, active) = match self.loop_mode {
            LoopMode::Once => self.advance_once(dt),
            LoopMode::Count(total_plays) => self.advance_counted(dt, total_plays),
            LoopMode::Forever => self.advance_repeating(dt, false),
            LoopMode::Alternate => self.advance_repeating(dt, true),
        };
        (Some(value), active)
    }

    fn advance_once(&mut self, dt: f64) -> (T, bool) {
        let value = self.motion.update(dt);
        let active = self.motion.is_running() && !self.motion.is_finished();
        (value, active)
    }

    fn advance_counted(&mut self, dt: f64, total_plays: u64) -> (T, bool) {
        if !self.motion.supports_loops() {
            return self.advance_once(dt);
        }
        let remaining_plays = total_plays.saturating_sub(self.completed_plays);
        if remaining_plays == 0 {
            self.finish_forward(total_plays);
            return (self.motion.value(), false);
        }

        let (duration, elapsed) = match &self.motion {
            AnimatedMotion::Timed(animation) => (animation.duration, animation.elapsed + dt),
            AnimatedMotion::Keyframes(_) | AnimatedMotion::Spring(_) => {
                return self.advance_once(dt);
            }
        };
        if elapsed >= duration * remaining_plays as f64 {
            self.finish_forward(total_plays);
            return (self.motion.value(), false);
        }

        let crossed = (elapsed / duration).floor() as u64;
        self.completed_plays = self.completed_plays.saturating_add(crossed);
        self.set_leg(self.original_from, self.original_to, elapsed % duration);
        (self.motion.value(), true)
    }

    fn advance_repeating(&mut self, dt: f64, alternate: bool) -> (T, bool) {
        let (duration, elapsed) = match &self.motion {
            AnimatedMotion::Timed(animation) => (animation.duration, animation.elapsed + dt),
            AnimatedMotion::Keyframes(_) | AnimatedMotion::Spring(_) => {
                return self.advance_once(dt);
            }
        };
        let crossed = (elapsed / duration).floor() as u64;
        self.completed_plays = self.completed_plays.saturating_add(crossed);
        let reverse_leg = alternate && self.completed_plays % 2 == 1;
        if reverse_leg {
            self.set_leg(self.original_to, self.original_from, elapsed % duration);
        } else {
            self.set_leg(self.original_from, self.original_to, elapsed % duration);
        }
        (self.motion.value(), true)
    }

    fn set_leg(&mut self, from: T, to: T, elapsed: f64) {
        if let AnimatedMotion::Timed(animation) = &mut self.motion {
            animation.from = from;
            animation.to = to;
            animation.elapsed = elapsed;
            animation.running = true;
        }
    }

    fn finish_forward(&mut self, completed_plays: u64) {
        match &mut self.motion {
            AnimatedMotion::Timed(animation) => {
                animation.from = self.original_from;
                animation.to = self.original_to;
                animation.elapsed = animation.duration;
                animation.running = false;
            }
            AnimatedMotion::Keyframes(animation) => animation.stop(),
            AnimatedMotion::Spring(animation) => animation.stop(),
        }
        self.completed_plays = completed_plays;
    }

    fn restart(&mut self, now: Instant) {
        self.completed_plays = 0;
        if matches!(self.loop_mode, LoopMode::Count(0)) {
            self.finish_forward(0);
        } else {
            if self.motion.supports_loops() {
                self.set_leg(self.original_from, self.original_to, 0.0);
            } else {
                self.motion.restart();
            }
            self.arm_delay(now);
        }
    }

    fn reverse(&mut self, now: Instant) {
        std::mem::swap(&mut self.original_from, &mut self.original_to);
        self.completed_plays = 0;
        if self.motion.supports_loops() {
            self.set_leg(self.original_from, self.original_to, 0.0);
        } else {
            self.motion.reverse();
        }
        self.arm_delay(now);
    }

    fn configure_loop(&mut self, loop_mode: LoopMode, now: Instant) -> Option<T> {
        if !self.motion.supports_loops() {
            return None;
        }
        self.loop_mode = loop_mode;
        self.completed_plays = 0;
        if matches!(loop_mode, LoopMode::Count(0)) {
            self.delay_state = DelayState::None;
            self.finish_forward(0);
            return Some(self.motion.value());
        }
        if matches!(self.delay_state, DelayState::None) && self.motion.is_finished() {
            self.restart(now);
            return Some(self.motion.value());
        }
        None
    }

    fn pause(&mut self, now: Instant) {
        if let DelayState::Waiting(deadline) = self.delay_state {
            self.delay_state = DelayState::Paused(deadline.saturating_duration_since(now));
        }
        self.motion.pause();
    }

    fn resume(&mut self, now: Instant) {
        if let DelayState::Paused(remaining) = self.delay_state {
            self.delay_state = DelayState::Waiting(deadline_after(now, remaining));
        }
        self.motion.resume();
    }

    fn stop(&mut self) {
        self.delay_state = DelayState::None;
        self.motion.stop();
    }

    fn value(&self) -> T {
        self.motion.value()
    }

    fn arm_delay(&mut self, now: Instant) {
        self.delay_state = if self.delay.is_zero() {
            DelayState::None
        } else {
            DelayState::Waiting(deadline_after(now, self.delay))
        };
    }
}

fn normalized_delay(seconds: f64) -> Duration {
    if seconds.is_nan() || seconds <= 0.0 {
        return Duration::ZERO;
    }
    Duration::try_from_secs_f64(seconds).unwrap_or(Duration::MAX)
}

fn deadline_after(now: Instant, delay: Duration) -> Instant {
    now.checked_add(delay).unwrap_or_else(|| {
        // `Instant` 没有公开最大值；极端输入取当前平台仍可表达的最远时刻。
        let mut candidate = delay;
        loop {
            candidate /= 2;
            if let Some(deadline) = now.checked_add(candidate) {
                return deadline;
            }
        }
    })
}

impl<T: Animatable + Sync> AnimatedSource for AnimatedInner<T> {
    fn work_id(&self) -> ComponentId {
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
        let (value, active) = {
            let mut playback = self
                .playback
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(playback) = playback.as_mut() else {
                return false;
            };
            playback.advance(now, dt)
        };
        if let Some(value) = value {
            self.current.set(value);
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
                work_id: ComponentId::new(NEXT_ANIMATED_ID.fetch_add(1, Ordering::Relaxed)),
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
        self.animate_keyframes(frames, duration)?;
        Ok(self)
    }

    /// Erases the value type so this source can be scheduled by `AnimationGroup`.
    pub fn group_item(&self) -> AnimationGroupItem {
        self.into()
    }

    pub(crate) fn group_source_id(&self) -> ComponentId {
        self.inner.work_id
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
        let current = self.inner.current.get_untracked();
        let next = KeyframeAnimation::from_current(current, frames, duration)?;
        let value = next.value();
        *self
            .inner
            .playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Some(AnimatedPlayback::new_keyframes(next));
        self.inner.current.set(value);
        Ok(())
    }

    fn replace_playback(&self, target: T, duration: f64, easing: Easing, delay: Duration) {
        let from = self.inner.current.get_untracked();
        let mut next = Animation::new(from, target, duration).easing(easing);
        let immediate = delay.is_zero() && next.duration <= 0.0;
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
            (AnimatedMotion::Timed(_), LoopMode::Forever | LoopMode::Alternate) => {
                return GroupItemTiming::Unbounded;
            }
            (AnimatedMotion::Keyframes(animation), _) => animation.duration(),
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
