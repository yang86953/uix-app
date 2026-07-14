use crate::native::backends::windows::platform::WindowsPlatform;
use crate::native::backends::windows::tsf_text_store::*;
use crate::native::traits::IWindowManager;
use crate::tests::common::*;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::TextServices::{TS_AS_SEL_CHANGE, TS_AS_TEXT_CHANGE};
use windows::Win32::UI::TextServices::{TS_LF_READ, TS_LF_READWRITE, TS_LF_SYNC};

fn test_state() -> TsfStoreState {
    TsfStoreState::new(TsfEventSink {
        events: Arc::new(Mutex::new(VecDeque::new())),
        window_id: WindowId::new(1),
        hwnd: HWND(std::ptr::null_mut()),
    })
}

#[test]
fn replace_range_updates_selection_and_buffer() {
    let mut state = test_state();
    let change = state.replace_range(0, 0, &[b'z' as u16, b'h' as u16]);
    assert_eq!(state.utf16_string(), "zh");
    assert_eq!(change.acpNewEnd, 2);
    assert_eq!(state.sel_end, 2);
}

#[test]
fn advise_mask_constants_cover_text_and_selection() {
    assert_ne!(TS_AS_TEXT_CHANGE, 0);
    assert_ne!(TS_AS_SEL_CHANGE, 0);
}

#[test]
fn read_lock_queues_exactly_one_asynchronous_write_upgrade() {
    let mut state = test_state();
    assert_eq!(
        state.begin_lock(TS_LF_READ.0),
        TsfLockRequest::Grant(TsfLockKind::Read)
    );
    assert!(state.has_read_lock());
    assert!(!state.has_write_lock());

    assert_eq!(
        state.begin_lock(TS_LF_READWRITE.0),
        TsfLockRequest::PendingWrite
    );
    assert_eq!(
        state.begin_lock(TS_LF_READWRITE.0),
        TsfLockRequest::PendingWrite
    );
    assert_eq!(state.complete_lock(), Some(TsfLockKind::ReadWrite));
    assert!(state.has_write_lock());
    assert_eq!(state.complete_lock(), None);
    assert!(!state.has_read_lock());
}

#[test]
fn synchronous_upgrade_is_rejected_without_leaking_a_pending_lock() {
    let mut state = test_state();
    assert_eq!(
        state.begin_lock(TS_LF_READ.0),
        TsfLockRequest::Grant(TsfLockKind::Read)
    );
    assert_eq!(
        state.begin_lock(TS_LF_READWRITE.0 | TS_LF_SYNC),
        TsfLockRequest::RejectSynchronous
    );
    assert_eq!(state.complete_lock(), None);
    assert!(!state.has_read_lock());
}

#[test]
fn cursor_and_screen_extent_use_client_surface_in_screen_coordinates() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("TSF geometry test", 320, 240)
        .expect("create TSF geometry window");
    let hwnd = HWND(window.native_handle().native_window());
    let mut state = TsfStoreState::new(TsfEventSink {
        events: Arc::new(Mutex::new(VecDeque::new())),
        window_id: window.window_id(),
        hwnd,
    });
    state.set_cursor_rect(RECT {
        left: 12,
        top: 18,
        right: 14,
        bottom: 42,
    });

    let extent = state.screen_extent().expect("screen extent");
    let cursor = state.cursor_screen_rect().expect("cursor screen rect");
    assert_eq!(
        (extent.right - extent.left, extent.bottom - extent.top),
        (320, 240)
    );
    assert_eq!(cursor.left, extent.left + 12);
    assert_eq!(cursor.top, extent.top + 18);
    assert_eq!(cursor.right, extent.left + 14);
    assert_eq!(cursor.bottom, extent.top + 42);

    window.close().expect("close TSF geometry window");
}
