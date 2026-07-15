use crate::native::backends::windows::platform::WindowsPlatform;
use crate::native::backends::windows::tsf_text_store::*;
use crate::native::traits::event::{UiEvent, UiEventType};
use crate::native::traits::IWindowManager;
use crate::tests::common::*;
use windows::Win32::Foundation::{E_UNEXPECTED, HWND, RECT};
use windows::Win32::UI::TextServices::{
    ITextStoreACP, TS_AE_END, TS_AS_SEL_CHANGE, TS_AS_TEXT_CHANGE, TS_ATTRVAL, TS_E_INVALIDPOS,
    TS_LF_READ, TS_LF_READWRITE, TS_LF_SYNC, TS_RUNINFO, TS_SELECTIONSTYLE, TS_SELECTION_ACP,
};

fn test_event_sink() -> TsfEventSink {
    TsfEventSink {
        events: Arc::new(Mutex::new(VecDeque::new())),
        window_id: WindowId::new(1),
        hwnd: HWND(std::ptr::null_mut()),
    }
}

fn test_state() -> TsfStoreState {
    TsfStoreState::new(test_event_sink())
}

fn test_text_store() -> ITextStoreACP {
    test_text_store_with_state().0
}

fn test_text_store_with_state() -> (ITextStoreACP, TsfStoreHandle) {
    let (store, state) = TsfTextStore::create(test_event_sink());
    (store.to_interface(), state)
}

#[test]
fn replace_range_updates_selection_and_buffer() {
    let mut state = test_state();
    let change = state
        .replace_range(0, 0, &[b'z' as u16, b'h' as u16])
        .expect("valid replacement range");
    assert_eq!(state.utf16_string(), "zh");
    assert_eq!(change.acpNewEnd, 2);
    assert_eq!(state.sel_end, 2);
}

#[test]
fn replace_range_rejects_invalid_acp_without_mutation() {
    let mut state = test_state();
    state
        .replace_range(0, 0, &[b'z' as u16, b'h' as u16])
        .expect("seed text");

    let error = state
        .replace_range(-1, 3, &[b'x' as u16])
        .expect_err("range outside the document must fail");

    assert_eq!(error.code(), TS_E_INVALIDPOS);
    assert_eq!(state.utf16_string(), "zh");
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
fn thread_affine_store_reports_reentrant_borrow_instead_of_panicking() {
    let (_store, state) = TsfTextStore::create(test_event_sink());
    let write = state.write().expect("first mutable borrow");

    assert_eq!(
        state.read().err().map(|error| error.code()),
        Some(E_UNEXPECTED)
    );

    drop(write);
    assert!(state.read().is_ok());
}

#[test]
fn com_store_rejects_null_required_pointers() {
    let store = test_text_store();
    let mut position = 0;
    let error = unsafe { store.QueryInsert(0, 0, 0, std::ptr::null_mut(), &mut position) }
        .expect_err("QueryInsert requires both result pointers");
    assert_eq!(error.code(), windows::Win32::Foundation::E_INVALIDARG);

    let mut plain: [u16; 0] = [];
    let mut runs: [TS_RUNINFO; 0] = [];
    let mut run_count = 0;
    let error = unsafe {
        store.GetText(
            0,
            -1,
            &mut plain,
            std::ptr::null_mut(),
            &mut runs,
            &mut run_count,
            &mut position,
        )
    }
    .expect_err("GetText requires its scalar result pointers");
    assert_eq!(error.code(), windows::Win32::Foundation::E_INVALIDARG);

    let error = unsafe {
        store.InsertTextAtSelection(
            0,
            &[],
            std::ptr::null_mut(),
            &mut position,
            std::ptr::null_mut(),
        )
    }
    .expect_err("queried insertion requires range result pointers");
    assert_eq!(error.code(), windows::Win32::Foundation::E_INVALIDARG);

    let mut found = false.into();
    let mut found_offset = 0;
    let error = unsafe {
        store.FindNextAttrTransition(
            0,
            0,
            &[],
            0,
            std::ptr::null_mut(),
            &mut found,
            &mut found_offset,
        )
    }
    .expect_err("attribute transition requires all result pointers");
    assert_eq!(error.code(), windows::Win32::Foundation::E_INVALIDARG);

    let mut attributes: [TS_ATTRVAL; 0] = [];
    let error = unsafe { store.RetrieveRequestedAttrs(&mut attributes, std::ptr::null_mut()) }
        .expect_err("attribute retrieval requires fetched count");
    assert_eq!(error.code(), windows::Win32::Foundation::E_INVALIDARG);

    let error = unsafe { store.GetACPFromPoint(0, std::ptr::null(), 0) }
        .expect_err("point query requires an input point");
    assert_eq!(error.code(), windows::Win32::Foundation::E_INVALIDARG);
}

#[test]
fn com_store_rejects_invalid_acp_ranges_without_clamping() {
    let (store, state) = test_text_store_with_state();
    {
        let mut state = state.write().expect("seed store state");
        state
            .replace_range(0, 0, &[b'z' as u16, b'h' as u16])
            .expect("seed text");
        assert_eq!(
            state.begin_lock(TS_LF_READWRITE.0),
            TsfLockRequest::Grant(TsfLockKind::ReadWrite)
        );
    }

    let mut result_start = 0;
    let mut result_end = 0;
    let error = unsafe { store.QueryInsert(-1, 0, 0, &mut result_start, &mut result_end) }
        .expect_err("QueryInsert must reject an invalid start");
    assert_eq!(error.code(), windows::Win32::Foundation::E_INVALIDARG);

    let selection = TS_SELECTION_ACP {
        acpStart: 0,
        acpEnd: 3,
        style: TS_SELECTIONSTYLE {
            ase: TS_AE_END,
            fInterimChar: false.into(),
        },
    };
    let error = unsafe { store.SetSelection(&[selection]) }
        .expect_err("SetSelection must reject an out-of-document end");
    assert_eq!(error.code(), TS_E_INVALIDPOS);

    let error = unsafe { store.SetText(0, 0, 3, &[]) }
        .expect_err("SetText must reject an out-of-document end");
    assert_eq!(error.code(), TS_E_INVALIDPOS);

    let mut plain = [0u16; 2];
    let mut copied = 0;
    let mut runs: [TS_RUNINFO; 0] = [];
    let mut run_count = 0;
    let mut next = 0;
    let error = unsafe {
        store.GetText(
            -1,
            -1,
            &mut plain,
            &mut copied,
            &mut runs,
            &mut run_count,
            &mut next,
        )
    }
    .expect_err("GetText must reject an invalid start");
    assert_eq!(error.code(), TS_E_INVALIDPOS);

    let mut rect = RECT::default();
    let mut clipped = false.into();
    let error = unsafe { store.GetTextExt(0, 0, 3, &mut rect, &mut clipped) }
        .expect_err("GetTextExt must reject an out-of-document end");
    assert_eq!(error.code(), TS_E_INVALIDPOS);

    let mut state = state.write().expect("inspect store state");
    assert_eq!(state.utf16_string(), "zh");
    assert_eq!(state.sel_end, 2);
    assert_eq!(state.complete_lock(), None);
}

#[test]
fn tsf_sink_enqueues_before_waking_a_poisoned_event_queue() {
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let poisoned = Arc::clone(&events);
    let _ = std::thread::spawn(move || {
        let _queue = poisoned.lock().expect("lock queue before poisoning");
        panic!("poison TSF queue for recovery test");
    })
    .join();
    let window_id = WindowId::new(77);
    let sink = TsfEventSink {
        events: Arc::clone(&events),
        window_id,
        hwnd: HWND(std::ptr::null_mut()),
    };

    sink.push(vec![UiEvent::ime_composition_start()]);

    let queue = events.lock().unwrap_or_else(|error| error.into_inner());
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].window_id, Some(window_id));
    assert_eq!(queue[0].type_, UiEventType::ImeCompositionStart);
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
