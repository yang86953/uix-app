use uix::core::Rect;
use uix::native::traits::input::{KeyCode, KeyMod, MouseButton, ScrollDirection};
use uix::native::traits::system::{MemoryInfo, StatusLevel};
use uix::native::{DamageRegion, PresentDamage};

#[test]
fn native_input_protocol_is_stable_at_the_layer_boundary() {
    let modifiers = KeyMod::CTRL | KeyMod::SHIFT;
    assert!(modifiers.contains(KeyMod::CTRL));
    assert!(modifiers.intersects(KeyMod::SHIFT));
    assert_eq!(KeyCode::Enter, KeyCode::Enter);
    assert_eq!(MouseButton::Left, MouseButton::Left);
    assert!(ScrollDirection::Both.can_scroll_x());
    assert!(ScrollDirection::Both.can_scroll_y());
}

#[test]
fn native_shared_contracts_are_consumable_without_backend_access() {
    let damage = DamageRegion::from_rect(Rect::new(1.0, 2.0, 30.0, 40.0));
    assert_eq!(damage.bounds(), Some(Rect::new(1.0, 2.0, 30.0, 40.0)));
    assert!(!PresentDamage::single(0, 0, 10, 10).is_full());

    let memory = MemoryInfo {
        total_bytes: 16,
        available_bytes: 8,
        process_working_set: 4,
        process_private_bytes: 2,
    };
    assert!(memory.total_bytes >= memory.available_bytes);
    assert_eq!(StatusLevel::Warning, StatusLevel::Warning);
}
