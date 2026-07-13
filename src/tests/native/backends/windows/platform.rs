use crate::tests::common::*;
use std::collections::{ BTreeMap };
use crate::native::shared::{OsEventSource, PlatformWindowCore, WindowState};
use crate::native::traits::event::{EventLoopWaker, UiEvent};
use crate::native::traits::*;
use crate::native::backends::windows::platform::*;
use crate::native::backends::windows::consts::{WM_SIZE, WM_CHAR, WM_IME_STARTCOMPOSITION, WM_IME_ENDCOMPOSITION, SIZE_RESTORED};
use crate::native::backends::windows::ffi::PostMessageW;
use crate::native::traits::event::{UiEventPayload, UiEventType};

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
