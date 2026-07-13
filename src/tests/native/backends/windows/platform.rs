use crate::native::backends::windows::consts::{
    SIZE_RESTORED, WM_CHAR, WM_IME_ENDCOMPOSITION, WM_IME_STARTCOMPOSITION, WM_SIZE,
};
use crate::native::backends::windows::ffi::PostMessageW;
use crate::native::backends::windows::platform::*;
use crate::native::shared::OsEventSource;
use crate::native::traits::event::{UiEventPayload, UiEventType};
use crate::native::traits::*;
use crate::tests::common::*;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

fn size_lparam(width: u16, height: u16) -> isize {
    (u32::from(width) | (u32::from(height) << 16)) as isize
}

#[test]
fn native_windows_keep_independent_state_and_route_resize_events() {
    let mut platform = WindowsPlatform::new();
    let mut first = platform
        .create_window("UIX multi-window route A", 320, 200)
        .expect("first native window");
    let mut second = platform
        .create_window("UIX multi-window route B", 480, 300)
        .expect("second native window");

    let first_id = first.window_id();
    let second_id = second.window_id();
    assert_ne!(first_id, second_id);
    assert_eq!(
        (first.properties().width(), first.properties().height()),
        (320, 200)
    );
    assert_eq!(
        (second.properties().width(), second.properties().height()),
        (480, 300)
    );

    let first_hwnd = first.native_handle().native_window();
    let second_hwnd = second.native_handle().native_window();
    assert!(!first_hwnd.is_null());
    assert!(!second_hwnd.is_null());

    platform.dispatch_pending();
    while platform.next_event().is_some() {}

    unsafe {
        assert_ne!(
            PostMessageW(first_hwnd, WM_SIZE, SIZE_RESTORED, size_lparam(321, 222)),
            0
        );
        assert_ne!(
            PostMessageW(second_hwnd, WM_SIZE, SIZE_RESTORED, size_lparam(654, 333),),
            0
        );
    }
    assert!(platform.dispatch_pending());

    let mut resize_events = BTreeMap::new();
    while let Some(event) = platform.next_event() {
        if event.type_ == UiEventType::WindowResize {
            let UiEventPayload::Resize(data) = event.payload else {
                panic!("WindowResize must carry ResizeData");
            };
            resize_events.insert(event.window_id.expect("routed window id"), data);
        }
    }

    let first_resize = resize_events.get(&first_id).expect("first resize event");
    let second_resize = resize_events.get(&second_id).expect("second resize event");
    assert_eq!((first_resize.width, first_resize.height), (321, 222));
    assert_eq!((second_resize.width, second_resize.height), (654, 333));
    assert_eq!(
        (first.properties().width(), first.properties().height()),
        (321, 222)
    );
    assert_eq!(
        (second.properties().width(), second.properties().height()),
        (654, 333)
    );

    second.close().expect("close secondary window");
    assert!(platform.dispatch_pending());
    first.close().expect("close primary window");
    assert!(platform.dispatch_pending());
}

#[test]
fn native_dwm_frame_opportunity_waits_for_present_and_keeps_window_identity() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX DWM frame opportunity", 64, 48)
        .expect("native window");
    let window_id = window.window_id();
    let token = FrameRequestToken::new(9, 27);

    window.show().expect("show native window");
    assert!(platform.dispatch_pending());
    while platform.next_event().is_some() {}

    assert!(window
        .request_native_frame(NativeFrameRequest::after_present(token))
        .expect("arm DWM frame request"));

    let quiet_until = Instant::now() + Duration::from_millis(20);
    while Instant::now() < quiet_until {
        assert!(platform.dispatch_timeout(Duration::from_millis(2)));
        while let Some(event) = platform.next_event() {
            assert_ne!(event.type_, UiEventType::FrameOpportunity);
        }
    }

    let pixels = vec![0xff20_4060; 64 * 48];
    window
        .presenter()
        .present(&pixels, 64, 48, PresentDamage::Full)
        .expect("present GDI frame");
    window
        .native_frame_presented(token)
        .expect("release DWM waiter after present");

    let deadline = Instant::now() + Duration::from_secs(1);
    let mut opportunity = None;
    while Instant::now() < deadline && opportunity.is_none() {
        assert!(platform.dispatch_timeout(Duration::from_millis(20)));
        while let Some(event) = platform.next_event() {
            if event.type_ != UiEventType::FrameOpportunity {
                continue;
            }
            let UiEventPayload::FrameOpportunity(data) = event.payload else {
                panic!("FrameOpportunity must carry its token");
            };
            opportunity = Some((event.window_id, data.token));
        }
    }

    assert_eq!(opportunity, Some((Some(window_id), token)));
    window.close().expect("close native window");
    assert!(platform.dispatch_pending());
}

#[test]
fn native_show_hide_events_track_visibility_and_keep_window_identity() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX native visibility route", 96, 64)
        .expect("native window");
    let window_id = window.window_id();

    window.show().expect("show native window");
    assert!(window.is_visible());
    assert!(platform.dispatch_pending());
    let shown = std::iter::from_fn(|| platform.next_event())
        .any(|event| event.type_ == UiEventType::WindowShow && event.window_id == Some(window_id));
    assert!(shown, "WM_SHOWWINDOW must route WindowShow to its window");

    window.hide().expect("hide native window");
    assert!(!window.is_visible());
    assert!(platform.dispatch_pending());
    let hidden = std::iter::from_fn(|| platform.next_event())
        .any(|event| event.type_ == UiEventType::WindowHide && event.window_id == Some(window_id));
    assert!(hidden, "WM_SHOWWINDOW must route WindowHide to its window");

    window.close().expect("close native window");
    assert!(platform.dispatch_pending());
}

#[test]
fn native_text_input_routes_ime_lifecycle_and_surrogate_pair() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX text input route", 320, 200)
        .expect("native window");
    let window_id = window.window_id();
    let hwnd = window.native_handle().native_window();
    assert!(!hwnd.is_null());

    assert!(platform.dispatch_pending());
    while platform.next_event().is_some() {}

    platform
        .text_input()
        .set_target_window(window_id, hwnd)
        .expect("target text input window");
    platform.text_input().start().expect("start text input");
    platform
        .text_input()
        .set_cursor_rect(Rect::new(12.0, 16.0, 2.0, 18.0))
        .expect("position IME candidate window");

    unsafe {
        assert_ne!(PostMessageW(hwnd, WM_IME_STARTCOMPOSITION, 0, 0), 0);
        assert_ne!(PostMessageW(hwnd, WM_CHAR, 0xD83D, 0), 0);
        assert_ne!(PostMessageW(hwnd, WM_CHAR, 0xDE00, 0), 0);
        assert_ne!(PostMessageW(hwnd, WM_IME_ENDCOMPOSITION, 0, 0), 0);
    }
    assert!(platform.dispatch_pending());

    // TSF 会话激活后 IMM32 composition 消息被跳过（避免双发）；WM_CHAR 仍走 TextInput。
    let events: Vec<_> = std::iter::from_fn(|| platform.next_event()).collect();
    assert_eq!(
        events.iter().map(|event| event.type_).collect::<Vec<_>>(),
        vec![UiEventType::TextInput]
    );
    assert!(events
        .iter()
        .all(|event| event.window_id == Some(window_id)));
    let UiEventPayload::TextInput(text) = &events[0].payload else {
        panic!("expected text payload");
    };
    assert_eq!(text.text, "😀");

    platform.text_input().stop().expect("stop text input");
    window.close().expect("close native window");
    assert!(platform.dispatch_pending());
}
