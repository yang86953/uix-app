use super::*;

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
