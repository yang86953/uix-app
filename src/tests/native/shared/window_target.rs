use crate::core::WindowId;
use crate::native::shared::window_target::SurfaceWindowTargets;

#[test]
fn pointer_and_keyboard_focus_follow_their_own_surfaces() {
    let first = WindowId::new(1);
    let second = WindowId::new(2);
    let mut targets = SurfaceWindowTargets::default();
    targets.register_surface(101, first);
    targets.register_surface(202, second);

    assert_eq!(targets.pointer_enter(101), Some(first));
    assert_eq!(targets.keyboard_enter(202), Some(second));
    assert_eq!(targets.pointer_target(), Some(first));
    assert_eq!(targets.keyboard_target(), Some(second));

    assert!(!targets.pointer_leave(202));
    assert!(!targets.keyboard_leave(101));
    assert_eq!(targets.pointer_target(), Some(first));
    assert_eq!(targets.keyboard_target(), Some(second));

    assert!(targets.pointer_leave(101));
    assert!(targets.keyboard_leave(202));
    assert_eq!(targets.pointer_target(), None);
    assert_eq!(targets.keyboard_target(), None);
}

#[test]
fn unknown_enter_and_surface_removal_cannot_reuse_a_stale_target() {
    let first = WindowId::new(1);
    let second = WindowId::new(2);
    let mut targets = SurfaceWindowTargets::default();
    targets.register_surface(101, first);
    targets.register_surface(202, second);

    targets.pointer_enter(101);
    targets.keyboard_enter(101);
    assert_eq!(targets.pointer_enter(999), None);
    assert_eq!(targets.keyboard_enter(999), None);
    assert_eq!(targets.pointer_target(), None);
    assert_eq!(targets.keyboard_target(), None);

    targets.pointer_enter(202);
    targets.keyboard_enter(202);
    assert_eq!(targets.unregister_surface(202), Some(second));
    assert_eq!(targets.pointer_target(), None);
    assert_eq!(targets.keyboard_target(), None);
    assert_eq!(targets.window_for_surface(202), None);
}

#[test]
fn device_focus_can_be_cleared_without_removing_surface_registration() {
    let window = WindowId::new(7);
    let mut targets = SurfaceWindowTargets::default();
    targets.register_surface(707, window);
    targets.pointer_enter(707);
    targets.keyboard_enter(707);

    targets.clear_pointer_focus();
    targets.clear_keyboard_focus();

    assert_eq!(targets.pointer_target(), None);
    assert_eq!(targets.keyboard_target(), None);
    assert_eq!(targets.window_for_surface(707), Some(window));
}

#[test]
fn reused_surface_identity_requires_fresh_enter_events() {
    let first = WindowId::new(1);
    let second = WindowId::new(2);
    let mut targets = SurfaceWindowTargets::default();
    targets.register_surface(101, first);
    targets.pointer_enter(101);
    targets.keyboard_enter(101);

    targets.register_surface(101, second);

    assert_eq!(targets.pointer_target(), None);
    assert_eq!(targets.keyboard_target(), None);
    assert_eq!(targets.window_for_surface(101), Some(second));
    assert_eq!(targets.pointer_enter(101), Some(second));
    assert_eq!(targets.keyboard_enter(101), Some(second));
}
