use super::super::consts::{SIZE_RESTORED, WM_DPICHANGED, WM_SIZE};
use super::super::text_input::WindowsImeState;
use super::*;
use crate::core::{Errc, WindowId};
use crate::diagnostics::PendingFailureQueue;

#[test]
fn wnd_proc_failure_is_deferred_to_owner_boundary() {
    let queue = PendingFailureQueue::new();
    let mut platform = WindowsPlatform::new_with_pending(queue);
    let window = Rc::new(RefCell::new(WindowState {
        window_id: WindowId::new(1),
        ..WindowState::default()
    }));
    let ime = RefCell::new(WindowsImeState::default());

    platform.handle_message(std::ptr::null_mut(), &window, &ime, WM_DPICHANGED, 0, 0);

    let Some(error) = platform.take_pending_failure() else {
        panic!("WM_DPICHANGED failure must be queued");
    };
    assert_eq!(error.code(), Errc::InvalidArgument);
    assert!(error.message().contains("WM_DPICHANGED"));
    assert!(platform.take_pending_failure().is_none());
}

#[test]
fn custom_chrome_failure_is_deferred_to_owner_boundary() {
    let queue = PendingFailureQueue::new();
    let mut platform = WindowsPlatform::new_with_pending(queue);
    let window = Rc::new(RefCell::new(WindowState {
        window_id: WindowId::new(1),
        ..WindowState::default()
    }));
    let ime = RefCell::new(WindowsImeState::default());

    // The invalid HWND makes the custom-chrome Win32/DWM path fail without
    // creating a native window; WM_SIZE must only enqueue the typed error.
    platform.handle_message(
        1usize as *mut std::ffi::c_void,
        &window,
        &ime,
        WM_SIZE,
        SIZE_RESTORED as usize,
        0,
    );

    let Some(error) = platform.take_pending_failure() else {
        panic!("custom chrome failure must be queued");
    };
    assert_eq!(error.code(), Errc::PlatformError);
    assert!(error.message().contains("custom chrome:"));
    assert!(platform.take_pending_failure().is_none());
}
