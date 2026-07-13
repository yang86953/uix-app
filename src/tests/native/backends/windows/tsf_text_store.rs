use crate::native::backends::windows::tsf_text_store::*;
use crate::tests::common::*;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::TextServices::{TS_AS_SEL_CHANGE, TS_AS_TEXT_CHANGE};

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
