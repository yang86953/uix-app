//! Message / Notification 共用的逐项动画与到期调度。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::core::Point;
use crate::ui::animation::TransitionPlayer;
use crate::ui::state::{State, StateSlotId};
use crate::ui::AnimationConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ToastKey {
    Local(u64),
    External(u64),
}

#[derive(Clone)]
pub(crate) struct ToastRequest<T> {
    key: ToastKey,
    item: T,
    display_duration: Option<Duration>,
}

pub(crate) struct ToastQueue<T> {
    state: State<Vec<ToastRequest<T>>>,
    next_id: Arc<AtomicU64>,
}

impl<T> Clone for ToastQueue<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            next_id: Arc::clone(&self.next_id),
        }
    }
}

impl<T> Default for ToastQueue<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ToastQueue<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub(crate) fn new() -> Self {
        Self {
            state: State::new(Vec::new()),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    pub(crate) fn push(&self, item: T, duration_ms: u64) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.state.update(|items| {
            items.push(ToastRequest {
                key: ToastKey::Local(id),
                item,
                display_duration: display_duration(duration_ms),
            });
        });
        id
    }

    pub(crate) fn replace_external<I>(&self, items: I)
    where
        I: IntoIterator<Item = (u64, T, u64)>,
    {
        self.state.set(
            items
                .into_iter()
                .map(|(id, item, duration_ms)| ToastRequest {
                    key: ToastKey::External(id),
                    item,
                    display_duration: display_duration(duration_ms),
                })
                .collect(),
        );
    }

    pub(crate) fn remove_local(&self, id: u64) -> bool {
        self.remove_keys(&[ToastKey::Local(id)]) > 0
    }

    pub(crate) fn remove_keys(&self, keys: &[ToastKey]) -> usize {
        if keys.is_empty() {
            return 0;
        }
        let mut items = self.state.get();
        let before = items.len();
        items.retain(|request| !keys.contains(&request.key));
        let removed = before.saturating_sub(items.len());
        if removed > 0 {
            self.state.set(items);
        }
        removed
    }

    pub(crate) fn clear(&self) {
        if !self.state.get().is_empty() {
            self.state.set(Vec::new());
        }
    }

    pub(crate) fn values(&self) -> Vec<T> {
        self.state
            .get()
            .into_iter()
            .map(|request| request.item)
            .collect()
    }

    pub(crate) fn len(&self) -> usize {
        self.state.get().len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn generation(&self) -> u64 {
        self.state.generation()
    }

    pub(crate) fn slot_id(&self) -> StateSlotId {
        self.state.slot_id()
    }

    fn requests(&self) -> Vec<ToastRequest<T>> {
        self.state.get()
    }
}

fn display_duration(duration_ms: u64) -> Option<Duration> {
    (duration_ms > 0).then(|| Duration::from_millis(duration_ms))
}

pub(crate) struct ToastMotion<T> {
    entries: Vec<ToastMotionEntry<T>>,
    observed_slot: Option<StateSlotId>,
    observed_generation: u64,
    timer_id: u32,
}

impl<T> Default for ToastMotion<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            observed_slot: None,
            observed_generation: 0,
            timer_id: 1,
        }
    }
}

impl<T> ToastMotion<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub(crate) fn sync(
        &mut self,
        queue: &ToastQueue<T>,
        enter_animation: AnimationConfig,
        leave_animation: AnimationConfig,
    ) -> bool {
        let slot = queue.slot_id();
        let generation = queue.generation();
        if self.observed_slot == Some(slot) && self.observed_generation == generation {
            return false;
        }

        let requests = queue.requests();
        let mut changed = false;
        let mut timer_changed = false;

        for entry in &mut self.entries {
            if let Some(request) = requests.iter().find(|request| request.key == entry.key) {
                entry.item = request.item.clone();
                entry.display_duration = request.display_duration;
                if entry.is_leaving() {
                    entry.phase = ToastPhase::Entering(TransitionPlayer::new(enter_animation));
                    changed = true;
                }
            } else if !entry.is_leaving() {
                timer_changed |= entry.is_holding_with_deadline();
                entry.phase = ToastPhase::Leaving(TransitionPlayer::new(leave_animation));
                changed = true;
            }
        }

        for request in requests {
            if self.entries.iter().any(|entry| entry.key == request.key) {
                continue;
            }
            self.entries.push(ToastMotionEntry {
                key: request.key,
                item: request.item,
                display_duration: request.display_duration,
                phase: ToastPhase::Entering(TransitionPlayer::new(enter_animation)),
            });
            changed = true;
        }

        self.observed_slot = Some(slot);
        self.observed_generation = generation;
        if timer_changed {
            self.bump_timer_id();
        }
        changed
    }

    pub(crate) fn update(&mut self, dt: f64) -> ToastMotionUpdate {
        let previous_len = self.entries.len();
        let mut changed = false;
        let mut timer_changed = false;

        for entry in &mut self.entries {
            match &mut entry.phase {
                ToastPhase::Entering(player) => {
                    player.update(dt);
                    changed = true;
                    if player.finished {
                        entry.phase = ToastPhase::Holding {
                            remaining: entry.display_duration,
                        };
                        timer_changed |= entry.display_duration.is_some();
                    }
                }
                ToastPhase::Holding { .. } => {}
                ToastPhase::Leaving(player) => {
                    player.update(dt);
                    changed = true;
                }
            }
        }

        let before_retain = self.entries.len();
        self.entries.retain(|entry| !entry.leave_finished());
        if self.entries.len() != before_retain {
            changed = true;
        }
        if timer_changed {
            self.bump_timer_id();
        }

        ToastMotionUpdate {
            active: self.entries.iter().any(ToastMotionEntry::is_animating),
            changed,
            previous_len,
            current_len: self.entries.len(),
        }
    }

    pub(crate) fn fire_timer(
        &mut self,
        timer_id: u32,
        leave_animation: AnimationConfig,
    ) -> Vec<ToastKey> {
        if timer_id != self.timer_id {
            return Vec::new();
        }
        let Some(elapsed) = self.next_delay() else {
            return Vec::new();
        };

        let mut expired = Vec::new();
        for entry in &mut self.entries {
            let ToastPhase::Holding {
                remaining: Some(remaining),
            } = &mut entry.phase
            else {
                continue;
            };
            if *remaining <= elapsed {
                expired.push(entry.key);
                entry.phase = ToastPhase::Leaving(TransitionPlayer::new(leave_animation));
            } else {
                *remaining = remaining.saturating_sub(elapsed);
            }
        }
        self.bump_timer_id();
        expired
    }

    pub(crate) fn active_timer(&self) -> Option<(u64, Duration)> {
        self.next_delay()
            .map(|delay| (u64::from(self.timer_id), delay))
    }

    pub(crate) fn entries(&self) -> &[ToastMotionEntry<T>] {
        &self.entries
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn next_delay(&self) -> Option<Duration> {
        self.entries
            .iter()
            .filter_map(|entry| match entry.phase {
                ToastPhase::Holding {
                    remaining: Some(remaining),
                } => Some(remaining),
                _ => None,
            })
            .min()
    }

    fn bump_timer_id(&mut self) {
        self.timer_id = self.timer_id.wrapping_add(1).max(1);
    }
}

pub(crate) struct ToastMotionUpdate {
    pub(crate) active: bool,
    pub(crate) changed: bool,
    pub(crate) previous_len: usize,
    pub(crate) current_len: usize,
}

pub(crate) struct ToastMotionEntry<T> {
    key: ToastKey,
    item: T,
    display_duration: Option<Duration>,
    phase: ToastPhase,
}

impl<T> ToastMotionEntry<T> {
    pub(crate) fn key(&self) -> ToastKey {
        self.key
    }

    pub(crate) fn item(&self) -> &T {
        &self.item
    }

    pub(crate) fn opacity(&self) -> f32 {
        self.player().map_or(1.0, |player| player.opacity_progress)
    }

    pub(crate) fn offset(&self) -> Point {
        self.player()
            .map_or_else(|| Point::new(0.0, 0.0), |player| player.offset)
    }

    pub(crate) fn scale(&self) -> f32 {
        self.player().map_or(1.0, |player| player.scale)
    }

    pub(crate) fn offset_endpoints(&self) -> (Point, Point) {
        self.player().map_or(
            (Point::new(0.0, 0.0), Point::new(0.0, 0.0)),
            TransitionPlayer::offset_endpoints,
        )
    }

    pub(crate) fn is_leaving(&self) -> bool {
        matches!(self.phase, ToastPhase::Leaving(_))
    }

    fn player(&self) -> Option<&TransitionPlayer> {
        match &self.phase {
            ToastPhase::Entering(player) | ToastPhase::Leaving(player) => Some(player),
            ToastPhase::Holding { .. } => None,
        }
    }

    fn is_holding_with_deadline(&self) -> bool {
        matches!(self.phase, ToastPhase::Holding { remaining: Some(_) })
    }

    fn is_animating(&self) -> bool {
        matches!(self.phase, ToastPhase::Entering(_) | ToastPhase::Leaving(_))
    }

    fn leave_finished(&self) -> bool {
        matches!(&self.phase, ToastPhase::Leaving(player) if player.finished)
    }
}

enum ToastPhase {
    Entering(TransitionPlayer),
    Holding { remaining: Option<Duration> },
    Leaving(TransitionPlayer),
}
