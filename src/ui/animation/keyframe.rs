//! Typed single-value keyframe animation.

use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::ui::traits::Animatable;

use super::Easing;

type FinishCallback = Arc<Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>>;

/// A value and easing curve declared at one normalized timeline offset.
///
/// The easing curve belongs to the segment that starts at this keyframe; the
/// final keyframe's easing is therefore ignored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe<T> {
    pub offset: f64,
    pub value: T,
    pub easing: Easing,
}

impl<T> Keyframe<T> {
    pub const fn new(offset: f64, value: T) -> Self {
        Self {
            offset,
            value,
            easing: Easing::Linear,
        }
    }

    pub const fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}

/// Keyframe sequence construction failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyframeError {
    Empty,
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

/// A fixed-duration, typed single-value keyframe sequence.
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
    /// Builds a normalized sequence.
    ///
    /// Offsets are clamped to `[0, 1]`, then stably sorted. At duplicate
    /// offsets the last declaration wins. Missing boundaries repeat the first
    /// or last declared value.
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

        let first = normalized[0];
        if first.offset > 0.0 {
            normalized.insert(0, Keyframe::new(0.0, current.unwrap_or(first.value)));
        }
        let last = normalized[normalized.len() - 1];
        if last.offset < 1.0 {
            normalized.push(Keyframe::new(1.0, last.value));
        }

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

    /// Sets a one-shot callback shared by clones and fired globally at most once.
    pub fn on_finish<F>(mut self, callback: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        self.finish_callback = Some(Arc::new(Mutex::new(Some(Box::new(callback)))));
        self
    }

    pub fn update(&mut self, dt: f64) -> T {
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

    pub fn value(&self) -> T {
        let progress = if self.reversed {
            1.0 - self.progress()
        } else {
            self.progress()
        };
        self.sample(progress)
    }

    pub fn progress(&self) -> f64 {
        if self.duration > 0.0 {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    pub fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }

    pub const fn is_running(&self) -> bool {
        self.running
    }

    pub const fn duration(&self) -> f64 {
        self.duration
    }

    pub fn frames(&self) -> &[Keyframe<T>] {
        &self.frames
    }

    pub fn pause(&mut self) {
        self.running = false;
    }

    pub fn resume(&mut self) {
        if !self.is_finished() {
            self.running = true;
        }
    }

    pub fn stop(&mut self) {
        self.elapsed = self.duration;
        self.running = false;
    }

    pub fn restart(&mut self) {
        self.elapsed = 0.0;
        self.running = self.duration > 0.0;
    }

    pub fn reverse(&mut self) {
        self.reversed = !self.reversed;
        self.restart();
    }

    fn sample(&self, progress: f64) -> T {
        if progress <= 0.0 {
            return self.frames[0].value;
        }
        if progress >= 1.0 {
            return self.frames[self.frames.len() - 1].value;
        }

        let upper = self
            .frames
            .partition_point(|frame| frame.offset <= progress);
        let start = self.frames[upper.saturating_sub(1)];
        let end = self.frames[upper];
        let local = (progress - start.offset) / (end.offset - start.offset);
        T::lerp(start.value, end.value, start.easing.sample(local))
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
