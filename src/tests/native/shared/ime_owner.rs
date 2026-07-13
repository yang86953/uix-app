use crate::core::WindowId;
use crate::native::shared::ime_owner::{NativeImeOwner, NativeImeTarget};

fn target(window: u64, native_id: usize) -> NativeImeTarget {
    NativeImeTarget::new(WindowId::new(window), native_id)
}

#[test]
fn stale_selected_window_cannot_stop_the_new_ime_owner() {
    let first = target(1, 0x1000);
    let second = target(2, 0x2000);
    let mut owner = NativeImeOwner::default();

    owner.select(first);
    let first_session = owner.activate_selected().expect("activate first").current;
    owner.select(second);
    let switch = owner.activate_selected().expect("activate second");
    assert_eq!(switch.previous, Some(first_session));
    assert_eq!(switch.current.target, second);

    owner.select(first);
    assert_eq!(owner.deactivate_selected(), None);
    assert_eq!(owner.active(), Some(switch.current));
    assert!(owner.matches(second, switch.current.generation));
}

#[test]
fn recycled_native_identity_requires_a_new_window_and_generation() {
    let old = target(7, 0x1234);
    let replacement = target(8, 0x1234);
    let mut owner = NativeImeOwner::default();

    owner.select(old);
    let old_session = owner.activate_selected().expect("activate old").current;
    owner.forget_target(old);
    assert_eq!(owner.selected(), None);
    assert_eq!(owner.active(), None);
    assert!(!owner.matches(old, old_session.generation));

    owner.select(replacement);
    let replacement_session = owner
        .activate_selected()
        .expect("activate replacement")
        .current;
    assert_ne!(replacement_session.generation, old_session.generation);
    assert_eq!(replacement_session.target, replacement);
}

#[test]
fn activation_is_idempotent_but_failed_or_stopped_sessions_stay_invalid() {
    let target = target(3, 0x3000);
    let mut owner = NativeImeOwner::default();
    owner.select(target);

    let first = owner.activate_selected().expect("activate");
    let repeated = owner.activate_selected().expect("repeat activation");
    assert!(!repeated.changed);
    assert_eq!(repeated.current, first.current);

    assert!(owner.fail_activation(first.current));
    assert!(!owner.matches(target, first.current.generation));
    let retry = owner.activate_selected().expect("retry activation").current;
    assert_ne!(retry.generation, first.current.generation);
    assert_eq!(owner.deactivate_selected(), Some(retry));
    assert!(!owner.matches(target, retry.generation));
}

#[test]
fn explicit_shutdown_deactivates_whichever_window_is_current() {
    let target = target(4, 0x4000);
    let mut owner = NativeImeOwner::default();
    owner.select(target);
    let active = owner.activate_selected().expect("activate").current;

    assert_eq!(owner.deactivate_active(), Some(active));
    assert_eq!(owner.active(), None);
}
