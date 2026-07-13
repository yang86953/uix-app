use super::*;

#[test]
fn utf16_surrogate_pair_is_emitted_as_one_scalar() {
    let mut state = WindowsImeState::default();
    assert_eq!(state.decode_utf16_unit(0xD83D), None);
    assert_eq!(state.decode_utf16_unit(0xDE00).as_deref(), Some("😀"));
}

#[test]
fn invalid_surrogate_does_not_poison_following_text() {
    let mut state = WindowsImeState::default();
    assert_eq!(state.decode_utf16_unit(0xD83D), None);
    assert_eq!(
        state.decode_utf16_unit(u16::from(b'A')).as_deref(),
        Some("�A")
    );
    assert_eq!(
        state.decode_utf16_unit(u16::from(b'B')).as_deref(),
        Some("B")
    );
}

#[test]
fn composition_state_transitions_are_idempotent() {
    let mut state = WindowsImeState::default();
    assert!(state.begin_composition());
    assert!(!state.begin_composition());
    assert!(state.end_composition());
    assert!(!state.end_composition());
}

#[test]
fn start_without_selected_window_reports_error() {
    let mut input = WindowsTextInput::default();
    let err = input.start().expect_err("start without HWND must fail");
    assert_eq!(err.code(), Errc::InvalidOperation);
}

#[test]
fn start_on_real_window_activates_tsf_session() {
    if std::env::consts::OS != "windows" {
        return;
    }
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("TSF text input start", 320, 240)
        .expect("window");
    let hwnd = window.native_surface_ptr();
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let mut input = WindowsTextInput::new(Arc::clone(&events));
    input.set_hwnd(hwnd);
    input.set_window_id(WindowId::new(1));
    input.start().expect("start");
    assert!(
        input.tsf_client_id().unwrap_or(0) != 0,
        "start must activate TSF AssociateFocus session"
    );
    input.stop().expect("stop");
    assert!(input.tsf_client_id().is_none());
    window.close().expect("close");
}
