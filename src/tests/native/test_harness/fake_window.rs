use crate::native::test_harness::fake_window::*;
use crate::native::traits::window::IWindowManager;
use crate::tests::common::*;

#[test]
fn fake_window_manager_assigns_stable_incrementing_window_ids() {
    let mut manager = FakeWindowManager::new();
    let first = manager.create_window("first", 320, 200).unwrap();
    let second = manager.create_window("second", 640, 480).unwrap();

    assert_eq!(first.window_id(), WindowId::new(1));
    assert_eq!(second.window_id(), WindowId::new(2));
}
