/// A simple animatable float value.
pub struct Animation {
    pub from: f32,
    pub to: f32,
    pub duration: f32, // in seconds
    pub elapsed: f32,
    pub easing: Easing,
    pub running: bool,
    /// Called once when the animation finishes (elapsed >= duration).
    /// Receives the final `current_value()`.
    pub on_complete: Option<Box<dyn FnMut(f32) + Send>>,
}

// Manual impls: skip on_complete for Clone and Debug
impl Clone for Animation {
    fn clone(&self) -> Self {
        Self {
            from: self.from,
            to: self.to,
            duration: self.duration,
            elapsed: self.elapsed,
            easing: self.easing,
            running: self.running,
            on_complete: None, // callbacks are not cloned
        }
    }
}

impl std::fmt::Debug for Animation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Animation")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("duration", &self.duration)
            .field("elapsed", &self.elapsed)
            .field("easing", &self.easing)
            .field("running", &self.running)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}


impl Animation {
    pub fn new(from: f32, to: f32, duration: f32) -> Self {
        Self {
            from,
            to,
            duration,
            elapsed: 0.0,
            easing: Easing::Linear,
            running: true,
            on_complete: None,
        }
    }

    pub fn with_on_complete<F: FnMut(f32) + Send + 'static>(mut self, f: F) -> Self {
        self.on_complete = Some(Box::new(f));
        self
    }

    pub fn current_value(&self) -> f32 {
        let t = (self.elapsed / self.duration).min(1.0);
        let eased = match self.easing {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => t * (2.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        };
        self.from + (self.to - self.from) * eased
    }

    pub fn is_finished(&self) -> bool {
        self.elapsed >= self.duration
    }
}

/// Manages widget animations.
#[derive(Default)]
pub struct AnimationManager {
    animations: Vec<Animation>,
}

impl AnimationManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, anim: Animation) {
        self.animations.push(anim);
    }

    /// Advance all animations by `dt` seconds.
    /// Returns final values of animations that just completed this frame.
    pub fn update(&mut self, dt: f32) -> Vec<(usize, f32)> {
        let mut completed = Vec::new();

        for (i, anim) in self.animations.iter_mut().enumerate() {
            if anim.running {
                let was_finished = anim.elapsed >= anim.duration;
                anim.elapsed += dt;
                if !was_finished && anim.elapsed >= anim.duration {
                    // Just finished this frame
                    let final_val = anim.current_value();
                    completed.push((i, final_val));
                    if let Some(ref mut cb) = anim.on_complete {
                        cb(final_val);
                    }
                }
            }
        }

        // Remove finished animations (reverse order to preserve indices)
        for &(i, _) in completed.iter().rev() {
            self.animations.remove(i);
        }

        completed
    }

    pub fn animations(&self) -> &[Animation] {
        &self.animations
    }

    pub fn is_any_running(&self) -> bool {
        self.animations.iter().any(|a| a.running)
    }

    pub fn clear(&mut self) {
        self.animations.clear();
    }
}
