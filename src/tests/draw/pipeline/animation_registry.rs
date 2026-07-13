use crate::draw::pipeline::animation_registry::*;
use crate::tests::common::*;

#[test]
fn register_unregister_lifecycle() {
    let mut reg = AnimationRegistry::new();
    assert!(!reg.has_active());
    reg.register(NodeId::new(3));
    assert!(reg.has_active());
    assert!(reg.is_registered(NodeId::new(3)));
    reg.unregister(NodeId::new(3));
    assert!(!reg.has_active());
}

#[test]
fn active_ids_snapshot() {
    let mut reg = AnimationRegistry::new();
    reg.register(NodeId::new(1));
    reg.register(NodeId::new(5));
    let mut ids = reg.active_ids();
    ids.sort_unstable();
    assert_eq!(ids, vec![NodeId::new(1), NodeId::new(5)]);
}

#[test]
fn duplicate_register_is_idempotent() {
    let mut reg = AnimationRegistry::new();
    let id = NodeId::new(4);

    reg.register(id);
    reg.register(id);

    assert!(reg.has_active());
    assert!(reg.is_registered(id));
    assert_eq!(reg.active_ids(), vec![id]);
}

#[test]
fn clear_removes_all_active_nodes() {
    let mut reg = AnimationRegistry::new();

    reg.register(NodeId::new(1));
    reg.register(NodeId::new(5));

    reg.clear();

    assert!(!reg.has_active());
    assert!(reg.active_ids().is_empty());
    assert!(!reg.is_registered(NodeId::new(1)));
    assert!(!reg.is_registered(NodeId::new(5)));
}
