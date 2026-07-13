use crate::tests::common::*;
use windows::core::{
    implement, ComObject, Error as WinError, Interface, Ref, Result as WinResult, BOOL, GUID,
    HRESULT, PCWSTR, PWSTR,
};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::UI::TextServices::{
    ITextStoreACP, ITextStoreACPSink, ITextStoreACP_Impl, ITfCompositionView,
    ITfContextOwnerCompositionSink, ITfContextOwnerCompositionSink_Impl, TEXT_STORE_LOCK_FLAGS,
    TS_AE_END, TS_AS_SEL_CHANGE, TS_AS_TEXT_CHANGE, TS_E_NOLOCK, TS_E_SYNCHRONOUS, TS_IAS_NOQUERY,
    TS_IAS_QUERYONLY, TS_LF_READWRITE, TS_RT_PLAIN, TS_RUNINFO, TS_SELECTIONSTYLE,
    TS_SELECTION_ACP, TS_SS_NOHIDDENTEXT, TS_SS_TRANSITORY, TS_STATUS, TS_TEXTCHANGE,
};
use crate::native::backends::windows::tsf_session::tsf_composition_events;
use crate::native::shared::ime_events::ImeCompositionState;
use crate::native::traits::event::UiEvent;
use crate::native::backends::windows::tsf_text_store::*;

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
