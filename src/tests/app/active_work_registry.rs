use crate::app::active_work_registry::*;
use crate::tests::common::*;

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
fn parking_managed_animation_deadline_blocks_until_source_resync() {
    let now = Instant::now();
    let deadline = now + Duration::from_secs(5);
    let id = NodeId::new(7);
    let mut registry = ActiveWorkRegistry::new();
    registry.sync_animated_sources([(id, Some(deadline))]);

    registry.park_animated_deadlines();

    assert_eq!(registry.next_deadline(), None);
    assert_eq!(registry.animation_ids().collect::<Vec<_>>(), vec![id]);
    assert!(!registry.is_empty());

    registry.sync_animated_sources([(id, Some(deadline))]);
    assert_eq!(registry.next_deadline(), Some(deadline));
    assert!(registry.animation_ids().next().is_none());
}

#[test]
fn open_component_animation_takes_precedence_over_a_managed_deadline() {
    let now = Instant::now();
    let deadline = now + Duration::from_secs(5);
    let id = NodeId::new(7);
    let mut registry = ActiveWorkRegistry::new();
    registry.sync_animated_sources([(id, Some(deadline))]);

    registry.sync_component_animations([id]);

    assert_eq!(registry.next_deadline(), None);
    assert_eq!(registry.animation_ids().collect::<Vec<_>>(), vec![id]);

    registry.sync_component_animations([]);

    assert_eq!(registry.next_deadline(), Some(deadline));
    assert!(registry.animation_ids().next().is_none());
}

#[test]
fn managed_source_removal_keeps_the_overlapping_component_animation_open() {
    let now = Instant::now();
    let id = NodeId::new(7);
    let mut registry = ActiveWorkRegistry::new();
    registry.sync_animated_sources([(id, Some(now + Duration::from_secs(5)))]);
    registry.sync_component_animations([id]);

    registry.sync_animated_sources([]);

    assert_eq!(registry.next_deadline(), None);
    assert_eq!(registry.animation_ids().collect::<Vec<_>>(), vec![id]);

    registry.sync_component_animations([]);

    assert!(registry.is_empty());
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

    registry.sync_app_timers(vec![(2, now + Duration::from_millis(40))]);

    assert_eq!(
        registry.next_deadline(),
        Some(now + Duration::from_millis(40))
    );
}

#[test]
fn sync_app_timers_does_not_remove_external_app_timer_entries() {
    let now = Instant::now();
    let mut registry = ActiveWorkRegistry::new();
    registry.register(ActiveWorkKind::AppTimer(1), now + Duration::from_millis(10));

    registry.sync_app_timers(Vec::new());

    assert_eq!(
        registry.next_deadline(),
        Some(now + Duration::from_millis(10))
    );
}
