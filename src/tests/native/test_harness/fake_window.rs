use crate::tests::common::*;
use crate::core::error::Result;
use crate::native::test_harness::fake_graphics_context::FakeGraphicsContext;
use crate::native::test_harness::fake_presenter::FakePresenter;
use crate::native::traits::present::{ IPresenter };
use crate::native::traits::window::{
    INativeHandle, IWindowManager, IWindowProperties, PlatformWindow,
};
use crate::native::test_harness::fake_window::*;

#[test]
fn fake_window_manager_assigns_stable_incrementing_window_ids() {
    let mut manager = FakeWindowManager::new();
    let first = manager.create_window("first", 320, 200).unwrap();
    let second = manager.create_window("second", 640, 480).unwrap();

    assert_eq!(first.window_id(), WindowId::new(1));
    assert_eq!(second.window_id(), WindowId::new(2));
}
