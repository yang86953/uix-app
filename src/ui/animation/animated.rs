//! Declarative, runtime-driven animation values.

use super::{Animation, Easing};
use crate::core::ComponentId;
use crate::ui::foundation::state::State;
use crate::ui::traits::Animatable;
use std::cell::RefCell;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

static NEXT_ANIMATED_ID: AtomicUsize = AtomicUsize::new(1);

thread_local! {
    static ANIMATED_CAPTURE_STACK: RefCell<Vec<Vec<Arc<dyn AnimatedSource>>>> =
        const { RefCell::new(Vec::new()) };
}

pub(crate) trait AnimatedSource: Send + Sync {
    fn work_id(&self) -> ComponentId;
    fn bind_owner(&self, tree_scope: u64) -> bool;
    fn unbind_owner(&self, tree_scope: u64);
    fn is_active(&self) -> bool;
    fn advance(&self, dt: f64) -> bool;
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
    animation: Mutex<Option<Animation<T>>>,
    owner_tree_scope: Mutex<Option<u64>>,
}

impl<T: Animatable + Sync> AnimatedInner<T> {
    fn touch(&self) {
        self.current.set(self.current.get_untracked());
    }
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

    fn is_active(&self) -> bool {
        self.animation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .is_some_and(|animation| animation.running && !animation.is_finished())
    }

    fn advance(&self, dt: f64) -> bool {
        let (value, active) = {
            let mut animation = self
                .animation
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(animation) = animation.as_mut() else {
                return false;
            };
            if !animation.running {
                return false;
            }
            if dt <= 0.0 {
                return true;
            }
            let value = animation.update(dt);
            let active = animation.running && !animation.is_finished();
            (value, active)
        };
        self.current.set(value);
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
                animation: Mutex::new(None),
                owner_tree_scope: Mutex::new(None),
            }),
        }
    }

    /// Starts a transition and returns the same shared handle for builder-style use.
    pub fn to(self, target: T, duration: f64, easing: Easing) -> Self {
        self.animate_to(target, duration, easing);
        self
    }

    /// Retargets the shared value from its current position.
    pub fn animate_to(&self, target: T, duration: f64, easing: Easing) {
        let from = self.inner.current.get_untracked();
        let mut next = Animation::new(from, target, duration).easing(easing);
        let immediate = next.duration <= 0.0;
        if immediate {
            next.stop();
        }
        let value = next.value();
        *self
            .inner
            .animation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(next);
        if immediate {
            self.inner.current.set(value);
        } else {
            self.inner.touch();
        }
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
            .animation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map_or(1.0, Animation::progress)
    }

    /// Returns whether the current transition is at rest.
    pub fn is_finished(&self) -> bool {
        capture_animated_source(self.inner.clone());
        let _ = self.inner.current.get();
        self.inner
            .animation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .is_none_or(Animation::is_finished)
    }

    /// Pauses at the current value and stops requesting animation frames.
    pub fn pause(&self) {
        if let Some(animation) = self
            .inner
            .animation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_mut()
        {
            animation.pause();
        }
        self.inner.touch();
    }

    /// Resumes a paused, unfinished transition.
    pub fn resume(&self) {
        if let Some(animation) = self
            .inner
            .animation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_mut()
        {
            animation.resume();
        }
        self.inner.touch();
    }

    /// Jumps to the target and stops requesting frames.
    pub fn stop(&self) {
        let value = {
            let mut animation = self
                .inner
                .animation
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(animation) = animation.as_mut() else {
                return;
            };
            animation.stop();
            animation.value()
        };
        self.inner.current.set(value);
    }

    /// Restarts the current transition from its original start value.
    pub fn restart(&self) {
        let value = {
            let mut animation = self
                .inner
                .animation
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(animation) = animation.as_mut() else {
                return;
            };
            animation.restart();
            animation.value()
        };
        self.inner.current.set(value);
    }

    /// Swaps the current transition endpoints and restarts it.
    pub fn reverse(&self) {
        let value = {
            let mut animation = self
                .inner
                .animation
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let Some(animation) = animation.as_mut() else {
                return;
            };
            animation.reverse();
            animation.value()
        };
        self.inner.current.set(value);
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
                "animation",
                &self
                    .inner
                    .animation
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
            )
            .finish()
    }
}
