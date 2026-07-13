use super::*;
use crate::native::traits::event::UiEventType;

#[test]
fn tsf_composition_helpers_match_ime_events_shape() {
    let mut state = ImeCompositionState::default();
    let events = tsf_composition_events(&mut state, Some("zh"), None, false);
    assert!(state.active);
    assert_eq!(events[0].type_, UiEventType::ImeCompositionStart);
    assert_eq!(events[1].type_, UiEventType::ImeCompositionUpdate);

    let events = tsf_composition_events(&mut state, None, Some("中"), false);
    assert!(!state.active);
    assert_eq!(events[0].type_, UiEventType::ImeCompositionEnd);
    assert_eq!(events[1].type_, UiEventType::TextInput);
}

#[test]
fn activate_rejects_null_hwnd() {
    match TsfSession::activate(TsfActivateParams {
        hwnd: std::ptr::null_mut(),
        window_id: WindowId::new(1),
        events: Arc::new(Mutex::new(VecDeque::new())),
    }) {
        Err(err) => assert_eq!(err.code(), Errc::InvalidArgument),
        Ok(_) => panic!("null hwnd must fail"),
    }
}

#[test]
fn activate_on_real_window_with_text_store() {
    if std::env::consts::OS != "windows" {
        return;
    }
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("TSF store test", 320, 240)
        .expect("window");
    let hwnd = window.native_surface_ptr();
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let session = TsfSession::activate(TsfActivateParams {
        hwnd,
        window_id: WindowId::new(1),
        events,
    })
    .expect("TSF activate with store");
    assert_ne!(session.client_id(), 0);
    assert!(!session.composition_active());
    session.deactivate();
    window.close().expect("close");
}
