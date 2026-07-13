#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use crate::draw::pipeline::NodeId;

pub(crate) type TimerId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ActiveWorkKind {
    Animation(NodeId),
    Timer(TimerId),
    AppTimer(TimerId),
    ImeSession(NodeId),
}

#[derive(Debug, Default)]
pub(crate) struct ActiveWorkRegistry {
    entries: BTreeMap<ActiveWorkKind, Option<Instant>>,
    managed_timers: BTreeSet<TimerId>,
    managed_app_timers: BTreeSet<TimerId>,
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
        if let ActiveWorkKind::Timer(id) = kind {
            self.managed_timers.remove(&id);
        }
        if let ActiveWorkKind::AppTimer(id) = kind {
            self.managed_app_timers.remove(&id);
        }
        self.entries.remove(&kind).is_some()
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.entries.values().filter_map(|deadline| *deadline).min()
    }

    pub(crate) fn drain_due(&mut self, now: Instant) -> Vec<ActiveWorkKind> {
        let due: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(&kind, &deadline)| deadline.is_some_and(|d| d <= now).then_some(kind))
            .collect();
        for kind in &due {
            self.entries.remove(kind);
            if let ActiveWorkKind::Timer(id) = kind {
                self.managed_timers.remove(id);
            }
            if let ActiveWorkKind::AppTimer(id) = kind {
                self.managed_app_timers.remove(id);
            }
        }
        due
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn animation_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.entries.keys().filter_map(|kind| match *kind {
            ActiveWorkKind::Animation(id) => Some(id),
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
        let desired: BTreeMap<TimerId, Duration> = timers.into_iter().collect();
        self.entries.retain(|kind, _| match kind {
            ActiveWorkKind::Timer(id) if self.managed_timers.contains(id) => {
                desired.contains_key(id)
            }
            _ => true,
        });
        self.managed_timers.retain(|id| desired.contains_key(id));
        for (id, delay) in desired {
            self.entries
                .entry(ActiveWorkKind::Timer(id))
                .or_insert(Some(now + delay));
            self.managed_timers.insert(id);
        }
    }

    pub(crate) fn sync_app_timers<I>(&mut self, timers: I)
    where
        I: IntoIterator<Item = (TimerId, Instant)>,
    {
        let desired: BTreeMap<TimerId, Instant> = timers.into_iter().collect();
        self.entries.retain(|kind, _| match kind {
            ActiveWorkKind::AppTimer(id) if self.managed_app_timers.contains(id) => {
                desired.contains_key(id)
            }
            _ => true,
        });
        self.managed_app_timers
            .retain(|id| desired.contains_key(id));
        for (id, deadline) in desired {
            self.entries
                .insert(ActiveWorkKind::AppTimer(id), Some(deadline));
            self.managed_app_timers.insert(id);
        }
    }
}

#[cfg(test)]
#[path = "../tests/app/active_work_registry.rs"]
mod tests;
