use crate::tests::common::*;
use crate::ui::overlay::*;

#[test]
fn same_z_index_keeps_latest_entry_on_top() {
    let mut stack = OverlayStack::new();
    let first = ComponentId::new(1);
    let second = ComponentId::new(2);

    stack.push_entry(OverlayEntry::new(first, OverlayKind::Popover).z_index(10));
    stack.push_entry(OverlayEntry::new(second, OverlayKind::Tooltip).z_index(10));

    assert_eq!(stack.top().map(|entry| entry.owner()), Some(second));
}

#[test]
fn remove_for_owner_returns_removed_entries() {
    let mut stack = OverlayStack::new();
    let owner = ComponentId::new(1);
    let other = ComponentId::new(2);

    stack.push_entry(OverlayEntry::new(owner, OverlayKind::Modal).z_index(10));
    stack.push_entry(OverlayEntry::new(other, OverlayKind::Tooltip).z_index(20));

    let removed = stack.remove_for_owner(owner);

    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].owner(), owner);
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.top().map(|entry| entry.owner()), Some(other));
}

#[test]
fn hit_test_returns_topmost_entry_at_point() {
    let mut stack = OverlayStack::new();
    let lower = ComponentId::new(1);
    let upper = ComponentId::new(2);

    stack.push_entry(
        OverlayEntry::new(lower, OverlayKind::Popover)
            .bounds(Rect::new(0.0, 0.0, 100.0, 100.0))
            .z_index(10),
    );
    stack.push_entry(
        OverlayEntry::new(upper, OverlayKind::Tooltip)
            .bounds(Rect::new(20.0, 20.0, 100.0, 100.0))
            .z_index(20),
    );

    assert_eq!(
        stack.hit_test(30.0, 30.0).map(|entry| entry.owner()),
        Some(upper)
    );
}

#[test]
fn hit_test_ignores_entries_without_bounds() {
    let mut stack = OverlayStack::new();
    let unbounded = ComponentId::new(1);
    let bounded = ComponentId::new(2);

    stack.push_entry(OverlayEntry::new(unbounded, OverlayKind::Popover).z_index(20));
    stack.push_entry(
        OverlayEntry::new(bounded, OverlayKind::Tooltip)
            .bounds(Rect::new(0.0, 0.0, 50.0, 50.0))
            .z_index(10),
    );

    assert_eq!(
        stack.hit_test(10.0, 10.0).map(|entry| entry.owner()),
        Some(bounded)
    );
    assert!(stack.hit_test(80.0, 80.0).is_none());
}
