//! 动画播放状态机（单值过渡、循环、延迟）。

use std::fmt;
use std::time::{Duration, Instant};

use crate::ui::animation::traits::Animatable;
use crate::ui::animation::{Animation, KeyframeAnimation, SpringAnimation};

use super::AnimatedRegistration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LoopMode {
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
pub(super) enum AnimatedMotion<T: Animatable> {
    Timed(Animation<T>),
    Keyframes(KeyframeAnimation<T>),
    Spring(SpringAnimation<T>),
}

impl<T: Animatable> AnimatedMotion<T> {
    pub(super) fn value(&self) -> T {
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

    pub(super) fn progress(&self) -> f64 {
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

    pub(super) fn is_finished(&self) -> bool {
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

    pub(super) fn stop(&mut self) {
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

pub(super) struct AnimatedPlayback<T: Animatable> {
    pub(super) motion: AnimatedMotion<T>,
    original_from: T,
    original_to: T,
    pub(super) loop_mode: LoopMode,
    completed_plays: u64,
    pub(super) delay: Duration,
    delay_state: DelayState,
    finish_callback: Option<Box<dyn FnOnce() + Send + 'static>>,
}

impl<T: Animatable> AnimatedPlayback<T> {
    pub(super) fn new(animation: Animation<T>, loop_mode: LoopMode, delay: Duration, now: Instant) -> Self {
        let mut playback = Self {
            original_from: animation.from,
            original_to: animation.to,
            motion: AnimatedMotion::Timed(animation),
            loop_mode,
            completed_plays: 0,
            delay,
            delay_state: DelayState::None,
            finish_callback: None,
        };
        playback.arm_delay(now);
        playback
    }

    pub(super) fn new_spring(animation: SpringAnimation<T>) -> Self {
        Self {
            original_from: animation.from,
            original_to: animation.to,
            motion: AnimatedMotion::Spring(animation),
            loop_mode: LoopMode::Once,
            completed_plays: 0,
            delay: Duration::ZERO,
            delay_state: DelayState::None,
            finish_callback: None,
        }
    }

    pub(super) fn new_keyframes(animation: KeyframeAnimation<T>) -> Self {
        Self {
            original_from: animation.value(),
            original_to: animation.frames()[animation.frames().len() - 1].value,
            motion: AnimatedMotion::Keyframes(animation),
            loop_mode: LoopMode::Once,
            completed_plays: 0,
            delay: Duration::ZERO,
            delay_state: DelayState::None,
            finish_callback: None,
        }
    }

    fn is_active(&self) -> bool {
        matches!(self.delay_state, DelayState::None)
            && self.motion.is_running()
            && !matches!(self.loop_mode, LoopMode::Count(0))
            && !self.motion.is_finished()
    }

    pub(super) fn registration(&self) -> AnimatedRegistration {
        match self.delay_state {
            DelayState::Waiting(deadline) => AnimatedRegistration::Deadline(deadline),
            DelayState::None if self.is_active() => AnimatedRegistration::Open,
            DelayState::None | DelayState::Paused(_) => AnimatedRegistration::Inactive,
        }
    }

    pub(super) fn is_finished(&self) -> bool {
        matches!(self.delay_state, DelayState::None) && self.motion.is_finished()
    }

    pub(super) fn progress(&self) -> f64 {
        match self.delay_state {
            DelayState::None => self.motion.progress(),
            DelayState::Waiting(_) | DelayState::Paused(_) => 0.0,
        }
    }

    pub(super) fn group_progress(&self) -> f64 {
        if !matches!(self.delay_state, DelayState::None) {
            return 0.0;
        }
        match self.loop_mode {
            LoopMode::Count(0) => 1.0,
            LoopMode::Count(total) => {
                if self.completed_plays >= total && self.motion.is_finished() {
                    1.0
                } else {
                    ((self.completed_plays as f64 + self.motion.progress()) / total as f64)
                        .clamp(0.0, 1.0)
                }
            }
            LoopMode::Once | LoopMode::Forever | LoopMode::Alternate => self.motion.progress(),
        }
    }

    pub(super) fn advance(&mut self, now: Instant, dt: f64) -> (Option<T>, bool) {
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

    pub(super) fn restart(&mut self, now: Instant) {
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

    pub(super) fn reverse(&mut self, now: Instant) {
        std::mem::swap(&mut self.original_from, &mut self.original_to);
        self.completed_plays = 0;
        if self.motion.supports_loops() {
            self.set_leg(self.original_from, self.original_to, 0.0);
        } else {
            self.motion.reverse();
        }
        self.arm_delay(now);
    }

    pub(super) fn configure_loop(&mut self, loop_mode: LoopMode, now: Instant) -> Option<T> {
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

    pub(super) fn pause(&mut self, now: Instant) {
        if let DelayState::Waiting(deadline) = self.delay_state {
            self.delay_state = DelayState::Paused(deadline.saturating_duration_since(now));
        }
        self.motion.pause();
    }

    pub(super) fn resume(&mut self, now: Instant) {
        if let DelayState::Paused(remaining) = self.delay_state {
            self.delay_state = DelayState::Waiting(deadline_after(now, remaining));
        }
        self.motion.resume();
    }

    pub(super) fn stop(&mut self) {
        self.delay_state = DelayState::None;
        self.motion.stop();
    }

    pub(super) fn value(&self) -> T {
        self.motion.value()
    }

    fn arm_delay(&mut self, now: Instant) {
        self.delay_state = if self.delay.is_zero() {
            DelayState::None
        } else {
            DelayState::Waiting(deadline_after(now, self.delay))
        };
    }

    pub(super) fn replace_finish_callback(
        &mut self,
        callback: Box<dyn FnOnce() + Send + 'static>,
    ) -> Option<Box<dyn FnOnce() + Send + 'static>> {
        self.finish_callback = None;
        if self.is_finished() {
            Some(callback)
        } else {
            self.finish_callback = Some(callback);
            None
        }
    }

    pub(super) fn take_finished_callback(&mut self) -> Option<Box<dyn FnOnce() + Send + 'static>> {
        if self.is_finished() {
            self.finish_callback.take()
        } else {
            None
        }
    }
}

impl<T> fmt::Debug for AnimatedPlayback<T>
where
    T: Animatable + fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AnimatedPlayback")
            .field("motion", &self.motion)
            .field("original_from", &self.original_from)
            .field("original_to", &self.original_to)
            .field("loop_mode", &self.loop_mode)
            .field("completed_plays", &self.completed_plays)
            .field("delay", &self.delay)
            .field("delay_state", &self.delay_state)
            .field("has_finish_callback", &self.finish_callback.is_some())
            .finish()
    }
}

pub(super) fn normalized_delay(seconds: f64) -> Duration {
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

