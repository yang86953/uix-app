//! Declarative, runtime-driven animation values.

use super::{Animation, Easing};
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
struct AnimatedPlayback<T: Animatable> {
    animation: Animation<T>,
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
            animation,
            loop_mode,
            completed_plays: 0,
            delay,
            delay_state: DelayState::None,
        };
        playback.arm_delay(now);
        playback
    }

    fn is_active(&self) -> bool {
        matches!(self.delay_state, DelayState::None)
            && self.animation.running
            && self.animation.duration > 0.0
            && !matches!(self.loop_mode, LoopMode::Count(0))
            && !self.animation.is_finished()
    }

    fn registration(&self) -> AnimatedRegistration {
        match self.delay_state {
            DelayState::Waiting(deadline) => AnimatedRegistration::Deadline(deadline),
            DelayState::None if self.is_active() => AnimatedRegistration::Open,
            DelayState::None | DelayState::Paused(_) => AnimatedRegistration::Inactive,
        }
    }

    fn is_finished(&self) -> bool {
        matches!(self.delay_state, DelayState::None) && self.animation.is_finished()
    }

    fn progress(&self) -> f64 {
        match self.delay_state {
            DelayState::None => self.animation.progress(),
            DelayState::Waiting(_) | DelayState::Paused(_) => 0.0,
        }
    }

    fn advance(&mut self, now: Instant, dt: f64) -> (Option<T>, bool) {
        match self.delay_state {
            DelayState::Waiting(deadline) if deadline > now => return (None, false),
            DelayState::Waiting(_) => {
                self.delay_state = DelayState::None;
                self.animation.running = true;
                if self.animation.duration <= 0.0 {
                    self.finish_forward(self.completed_plays);
                    return (Some(self.animation.value()), false);
                }
                return (None, true);
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
            LoopMode::Once => {
                let value = self.animation.update(dt);
                let active = self.animation.running && !self.animation.is_finished();
                (value, active)
            }
            LoopMode::Count(total_plays) => self.advance_counted(dt, total_plays),
            LoopMode::Forever => self.advance_repeating(dt, false),
            LoopMode::Alternate => self.advance_repeating(dt, true),
        };
        (Some(value), active)
    }

    fn advance_counted(&mut self, dt: f64, total_plays: u64) -> (T, bool) {
        let remaining_plays = total_plays.saturating_sub(self.completed_plays);
        if remaining_plays == 0 {
            self.finish_forward(total_plays);
            return (self.animation.value(), false);
        }

        let duration = self.animation.duration;
        let elapsed = self.animation.elapsed + dt;
        if elapsed >= duration * remaining_plays as f64 {
            self.finish_forward(total_plays);
            return (self.animation.value(), false);
        }

        let crossed = (elapsed / duration).floor() as u64;
        self.completed_plays = self.completed_plays.saturating_add(crossed);
        self.set_leg(self.original_from, self.original_to, elapsed % duration);
        (self.animation.value(), true)
    }

    fn advance_repeating(&mut self, dt: f64, alternate: bool) -> (T, bool) {
        let duration = self.animation.duration;
        let elapsed = self.animation.elapsed + dt;
        let crossed = (elapsed / duration).floor() as u64;
        self.completed_plays = self.completed_plays.saturating_add(crossed);
        let reverse_leg = alternate && self.completed_plays % 2 == 1;
        if reverse_leg {
            self.set_leg(self.original_to, self.original_from, elapsed % duration);
        } else {
            self.set_leg(self.original_from, self.original_to, elapsed % duration);
        }
        (self.animation.value(), true)
    }

    fn set_leg(&mut self, from: T, to: T, elapsed: f64) {
        self.animation.from = from;
        self.animation.to = to;
        self.animation.elapsed = elapsed;
        self.animation.running = true;
    }

    fn finish_forward(&mut self, completed_plays: u64) {
        self.animation.from = self.original_from;
        self.animation.to = self.original_to;
        self.animation.elapsed = self.animation.duration;
        self.animation.running = false;
        self.completed_plays = completed_plays;
    }

    fn restart(&mut self, now: Instant) {
        self.completed_plays = 0;
        if matches!(self.loop_mode, LoopMode::Count(0)) {
            self.finish_forward(0);
        } else {
            self.set_leg(self.original_from, self.original_to, 0.0);
            self.arm_delay(now);
        }
    }

    fn reverse(&mut self, now: Instant) {
        std::mem::swap(&mut self.original_from, &mut self.original_to);
        self.restart(now);
    }

    fn configure_loop(&mut self, loop_mode: LoopMode, now: Instant) -> Option<T> {
        self.loop_mode = loop_mode;
        self.completed_plays = 0;
        if matches!(loop_mode, LoopMode::Count(0)) {
            self.delay_state = DelayState::None;
            self.finish_forward(0);
            return Some(self.animation.value());
        }
        if self.animation.duration > 0.0
            && matches!(self.delay_state, DelayState::None)
            && self.animation.is_finished()
        {
            self.restart(now);
            return Some(self.animation.value());
        }
        None
    }

    fn pause(&mut self, now: Instant) {
        if let DelayState::Waiting(deadline) = self.delay_state {
            self.delay_state = DelayState::Paused(deadline.saturating_duration_since(now));
        }
        self.animation.pause();
    }

    fn resume(&mut self, now: Instant) {
        if let DelayState::Paused(remaining) = self.delay_state {
            self.delay_state = DelayState::Waiting(deadline_after(now, remaining));
        }
        self.animation.resume();
    }

    fn stop(&mut self) {
        self.delay_state = DelayState::None;
        self.animation.stop();
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

/// A fixed-duration animation value advanced by the owning window's frame loop.
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

    /// Retargets the shared value from its current position.
    pub fn animate_to(&self, target: T, duration: f64, easing: Easing) {
        self.replace_playback(target, duration, easing, Duration::ZERO);
    }

    /// 在 `delay` 秒后从当前值开始过渡；等待期只登记 deadline，不申请动画帧。
    pub fn animate_to_after(&self, delay: f64, target: T, duration: f64, easing: Easing) {
        self.replace_playback(target, duration, easing, normalized_delay(delay));
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

    /// Repeats the forward transition indefinitely.
    pub fn loop_forever(self) -> Self {
        self.configure_loop(LoopMode::Forever);
        self
    }

    /// Alternates forward and reverse transitions indefinitely.
    pub fn loop_alternate(self) -> Self {
        self.configure_loop(LoopMode::Alternate);
        self
    }

    /// Plays the forward transition `count` times in total.
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
        if let Some(playback) = self
            .inner
            .playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_mut()
        {
            playback.pause(Instant::now());
        }
        self.inner.touch();
    }

    /// Resumes a paused, unfinished transition.
    pub fn resume(&self) {
        if let Some(playback) = self
            .inner
            .playback
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_mut()
        {
            playback.resume(Instant::now());
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
            playback.animation.value()
        };
        self.inner.current.set(value);
    }

    /// Restarts the current transition from its original start value.
    pub fn restart(&self) {
        let value = {
            let mut playback = self
                .inner
                .playback
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(playback) = playback.as_mut() else {
                return;
            };
            playback.restart(Instant::now());
            playback.animation.value()
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
            playback.animation.value()
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
