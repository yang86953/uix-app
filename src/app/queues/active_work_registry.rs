#![allow(dead_code)]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::draw::scene::NodeId;

pub(crate) type TimerId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ActiveWorkKind {
    Animation(NodeId),
    Timer(TimerId),
    AppTimer(TimerId),
    ImeSession(NodeId),
    GraphicsMaintenance,
}

#[derive(Debug, Default)]
pub(crate) struct ActiveWorkRegistry {
    entries: BTreeMap<ActiveWorkKind, Option<Instant>>,
    managed_animation_registrations: BTreeMap<NodeId, (Option<Instant>, bool)>,
    open_component_animations: BTreeMap<NodeId, bool>,
    managed_timers: BTreeMap<TimerId, bool>,
    managed_app_timers: BTreeMap<TimerId, bool>,
    timer_sync_marker: bool,
    app_timer_sync_marker: bool,
    animated_source_sync_marker: bool,
    component_animation_sync_marker: bool,
}

impl ActiveWorkRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn register(&mut self, kind: ActiveWorkKind, next_deadline: Instant) {
        self.entries.insert(kind, Some(next_deadline));
    }

    pub(crate) fn register_open(&mut self, kind: ActiveWorkKind) {
        self.entries.insert(kind, None);
    }

    pub(crate) fn unregister(&mut self, kind: ActiveWorkKind) -> bool {
        if let ActiveWorkKind::Animation(id) = kind {
            self.managed_animation_registrations.remove(&id);
            self.open_component_animations.remove(&id);
        }
        if let ActiveWorkKind::Timer(id) = kind {
            self.managed_timers.remove(&id);
        }
        if let ActiveWorkKind::AppTimer(id) = kind {
            self.managed_app_timers.remove(&id);
        }
        self.entries.remove(&kind).is_some()
    }

    pub(crate) fn sync_animated_sources<I>(&mut self, registrations: I)
    where
        I: IntoIterator<Item = (NodeId, Option<Instant>)>,
    {
        self.animated_source_sync_marker = !self.animated_source_sync_marker;
        let marker = self.animated_source_sync_marker;
        for (id, deadline) in registrations {
            self.managed_animation_registrations
                .insert(id, (deadline, marker));
        }
        let entries = &mut self.entries;
        let open_component_animations = &self.open_component_animations;
        self.managed_animation_registrations
            .retain(|&id, (_, seen_marker)| {
                if *seen_marker == marker {
                    return true;
                }
                let kind = ActiveWorkKind::Animation(id);
                if open_component_animations.contains_key(&id) {
                    entries.insert(kind, None);
                } else {
                    entries.remove(&kind);
                }
                false
            });
        for (&id, &(deadline, _)) in &self.managed_animation_registrations {
            let kind = ActiveWorkKind::Animation(id);
            if self.open_component_animations.contains_key(&id) {
                self.entries.insert(kind, None);
            } else {
                self.entries.insert(kind, deadline);
            }
        }
    }

    pub(crate) fn sync_component_animations<I>(&mut self, ids: I)
    where
        I: IntoIterator<Item = NodeId>,
    {
        self.component_animation_sync_marker = !self.component_animation_sync_marker;
        let marker = self.component_animation_sync_marker;
        for id in ids {
            self.open_component_animations.insert(id, marker);
        }
        let entries = &mut self.entries;
        let managed_animation_registrations = &self.managed_animation_registrations;
        self.open_component_animations.retain(|&id, seen_marker| {
            if *seen_marker == marker {
                return true;
            }
            let kind = ActiveWorkKind::Animation(id);
            if let Some(&(deadline, _)) = managed_animation_registrations.get(&id) {
                entries.insert(kind, deadline);
            } else {
                entries.remove(&kind);
            }
            false
        });
        for &id in self.open_component_animations.keys() {
            self.entries.insert(ActiveWorkKind::Animation(id), None);
        }
    }

    pub(crate) fn manages_animation(&self, id: NodeId) -> bool {
        self.managed_animation_registrations.contains_key(&id)
            || self.open_component_animations.contains_key(&id)
    }

    pub(crate) fn park_animated_deadlines(&mut self) {
        let managed = &self.managed_animation_registrations;
        for (kind, deadline) in &mut self.entries {
            if matches!(kind, ActiveWorkKind::Animation(id) if managed.contains_key(id)) {
                *deadline = None;
            }
        }
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.entries.values().filter_map(|deadline| *deadline).min()
    }

    #[cfg(test)]
    pub(crate) fn drain_due(&mut self, now: Instant) -> Vec<ActiveWorkKind> {
        let mut due = Vec::new();
        self.drain_due_into(now, &mut due);
        due
    }

    pub(crate) fn drain_due_into(&mut self, now: Instant, due: &mut Vec<ActiveWorkKind>) {
        due.clear();
        let managed_timers = &mut self.managed_timers;
        let managed_app_timers = &mut self.managed_app_timers;
        self.entries.retain(|kind, deadline| {
            if !deadline.is_some_and(|deadline| deadline <= now) {
                return true;
            }
            due.push(*kind);
            match *kind {
                ActiveWorkKind::Timer(id) => {
                    managed_timers.remove(&id);
                }
                ActiveWorkKind::AppTimer(id) => {
                    managed_app_timers.remove(&id);
                }
                _ => {}
            }
            false
        });
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn animation_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.entries
            .iter()
            .filter_map(|(kind, deadline)| match *kind {
                ActiveWorkKind::Animation(id) if deadline.is_none() => Some(id),
                _ => None,
            })
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn sync_timers<I>(&mut self, timers: I, now: Instant)
    where
        I: IntoIterator<Item = (TimerId, Duration)>,
    {
        self.timer_sync_marker = !self.timer_sync_marker;
        let marker = self.timer_sync_marker;
        for (id, delay) in timers {
            self.entries
                .entry(ActiveWorkKind::Timer(id))
                .or_insert(Some(now + delay));
            self.managed_timers.insert(id, marker);
        }
        let entries = &mut self.entries;
        self.managed_timers.retain(|&id, seen_marker| {
            if *seen_marker == marker {
                true
            } else {
                entries.remove(&ActiveWorkKind::Timer(id));
                false
            }
        });
    }

    pub(crate) fn sync_app_timers<I>(&mut self, timers: I)
    where
        I: IntoIterator<Item = (TimerId, Instant)>,
    {
        self.app_timer_sync_marker = !self.app_timer_sync_marker;
        let marker = self.app_timer_sync_marker;
        for (id, deadline) in timers {
            self.entries
                .insert(ActiveWorkKind::AppTimer(id), Some(deadline));
            self.managed_app_timers.insert(id, marker);
        }
        let entries = &mut self.entries;
        self.managed_app_timers.retain(|&id, seen_marker| {
            if *seen_marker == marker {
                true
            } else {
                entries.remove(&ActiveWorkKind::AppTimer(id));
                false
            }
        });
    }
}
