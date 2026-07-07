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
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn next_deadline_returns_earliest_registered_work() {
        let now = Instant::now();
        let mut registry = ActiveWorkRegistry::new();

        registry.register(
            ActiveWorkKind::Animation(NodeId::new(1)),
            now + Duration::from_millis(30),
        );
        registry.register(ActiveWorkKind::AppTimer(7), now + Duration::from_millis(10));
        registry.register(
            ActiveWorkKind::ImeSession(NodeId::new(2)),
            now + Duration::from_millis(20),
        );

        assert_eq!(
            registry.next_deadline(),
            Some(now + Duration::from_millis(10))
        );
    }

    #[test]
    fn register_replaces_existing_kind_deadline() {
        let now = Instant::now();
        let mut registry = ActiveWorkRegistry::new();

        registry.register(ActiveWorkKind::Timer(1), now + Duration::from_millis(50));
        registry.register(ActiveWorkKind::Timer(1), now + Duration::from_millis(5));

        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.next_deadline(),
            Some(now + Duration::from_millis(5))
        );
    }

    #[test]
    fn unregister_removes_matching_work() {
        let now = Instant::now();
        let mut registry = ActiveWorkRegistry::new();

        registry.register(ActiveWorkKind::Animation(NodeId::new(1)), now);
        registry.register(ActiveWorkKind::AppTimer(2), now);

        assert!(registry.unregister(ActiveWorkKind::Animation(NodeId::new(1))));
        assert!(!registry.unregister(ActiveWorkKind::Animation(NodeId::new(9))));
        assert_eq!(registry.drain_due(now), vec![ActiveWorkKind::AppTimer(2)]);
    }

    #[test]
    fn open_registration_has_no_deadline_and_is_not_drained() {
        let now = Instant::now();
        let mut registry = ActiveWorkRegistry::new();

        registry.register_open(ActiveWorkKind::ImeSession(NodeId::new(7)));

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.next_deadline(), None);
        assert!(registry.drain_due(now + Duration::from_secs(60)).is_empty());
        assert!(!registry.is_empty());
    }

    #[test]
    fn drain_due_returns_only_due_work_and_keeps_future_work() {
        let now = Instant::now();
        let mut registry = ActiveWorkRegistry::new();

        registry.register(
            ActiveWorkKind::Animation(NodeId::new(1)),
            now - Duration::from_millis(1),
        );
        registry.register(ActiveWorkKind::Timer(2), now);
        registry.register(ActiveWorkKind::AppTimer(3), now + Duration::from_millis(1));

        let due = registry.drain_due(now);

        assert_eq!(
            due,
            vec![
                ActiveWorkKind::Animation(NodeId::new(1)),
                ActiveWorkKind::Timer(2)
            ]
        );
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.next_deadline(),
            Some(now + Duration::from_millis(1))
        );
    }

    #[test]
    fn drain_due_empties_registry_when_all_work_is_due() {
        let now = Instant::now();
        let mut registry = ActiveWorkRegistry::new();

        registry.register(ActiveWorkKind::Timer(1), now);
        registry.register(ActiveWorkKind::ImeSession(NodeId::new(2)), now);

        assert_eq!(registry.drain_due(now).len(), 2);
        assert!(registry.is_empty());
        assert_eq!(registry.next_deadline(), None);
    }

    #[test]
    fn sync_timers_registers_new_and_removes_inactive_timers() {
        let now = Instant::now();
        let mut registry = ActiveWorkRegistry::new();
        registry.sync_timers(vec![(1, Duration::from_millis(10))], now);
        registry.register(
            ActiveWorkKind::Animation(NodeId::new(7)),
            now + Duration::from_millis(20),
        );

        registry.sync_timers(vec![(2, Duration::from_millis(30))], now);

        assert!(!registry.unregister(ActiveWorkKind::Timer(1)));
        assert!(registry.unregister(ActiveWorkKind::Animation(NodeId::new(7))));
        assert_eq!(
            registry.next_deadline(),
            Some(now + Duration::from_millis(30))
        );
    }

    #[test]
    fn sync_timers_does_not_remove_external_timer_entries() {
        let now = Instant::now();
        let mut registry = ActiveWorkRegistry::new();
        registry.register(ActiveWorkKind::Timer(1), now + Duration::from_millis(10));

        registry.sync_timers(Vec::new(), now);

        assert_eq!(
            registry.next_deadline(),
            Some(now + Duration::from_millis(10))
        );
    }

    #[test]
    fn sync_timers_keeps_existing_timer_deadline_stable() {
        let now = Instant::now();
        let mut registry = ActiveWorkRegistry::new();
        registry.register(ActiveWorkKind::Timer(1), now + Duration::from_millis(10));

        registry.sync_timers(vec![(1, Duration::from_millis(30))], now);

        assert_eq!(
            registry.next_deadline(),
            Some(now + Duration::from_millis(10))
        );
    }

    #[test]
    fn sync_app_timers_replaces_managed_app_timer_deadlines() {
        let now = Instant::now();
        let mut registry = ActiveWorkRegistry::new();
        registry.sync_app_timers(vec![(1, now + Duration::from_millis(10))]);
        registry.register(
            ActiveWorkKind::Animation(NodeId::new(7)),
            now + Duration::from_millis(20),
        );

        registry.sync_app_timers(vec![(2, now + Duration::from_millis(30))]);

        assert!(!registry.unregister(ActiveWorkKind::AppTimer(1)));
        assert!(registry.unregister(ActiveWorkKind::Animation(NodeId::new(7))));
        assert_eq!(
            registry.next_deadline(),
            Some(now + Duration::from_millis(30))
        );
    }
}
