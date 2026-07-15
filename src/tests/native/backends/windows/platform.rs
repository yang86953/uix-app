use crate::app::window_actions::configure_custom_title_bar;
use crate::native::backends::windows::consts::{
    GWL_EXSTYLE, GWL_STYLE, HTCAPTION, MONITOR_DEFAULTTONEAREST, SIZE_RESTORED, WM_CHAR,
    WM_DPICHANGED, WM_IME_ENDCOMPOSITION, WM_IME_STARTCOMPOSITION, WM_LBUTTONDBLCLK, WM_LBUTTONUP,
    WM_NCRBUTTONUP, WM_SIZE, WS_CAPTION, WS_EX_LAYERED, WS_THICKFRAME,
};
use crate::native::backends::windows::dpi::{
    dpi_for_window, logical_extent_to_physical, physical_extent_to_logical,
};
use crate::native::backends::windows::ffi::{
    GetMonitorInfoW as GetMonitorInfoRaw, GetWindowLongW,
    MonitorFromWindow as MonitorFromWindowRaw, PostMessageW,
};
use crate::native::backends::windows::platform::*;
use crate::native::backends::windows::window_ops::set_window_long_checked;
use crate::native::graphics::platform::windows::{drawable_size, query_client_rect};
use crate::native::shared::OsEventSource;
use crate::native::traits::event::{UiEventPayload, UiEventType};
use crate::native::traits::*;
use crate::tests::common::*;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, GetWindowDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, PeekMessageW, SendMessageW, MSG, PM_REMOVE,
};

fn size_lparam(width: u16, height: u16) -> isize {
    (u32::from(width) | (u32::from(height) << 16)) as isize
}

#[test]
fn native_event_queue_recovers_after_lock_poisoning() {
    let mut platform = WindowsPlatform::new();
    let poisoned = Arc::clone(&platform.event_queue);
    let _ = std::thread::spawn(move || {
        let _queue = poisoned.lock().expect("lock event queue before poisoning");
        panic!("poison Windows event queue for recovery test");
    })
    .join();
    let window_id = crate::core::WindowId::new(77);

    platform.push_event(window_id, UiEvent::close());

    let event = platform
        .next_event()
        .expect("poisoned event queue must remain usable");
    assert_eq!(event.window_id, Some(window_id));
    assert_eq!(event.type_, UiEventType::WindowClose);
}

#[test]
fn native_window_routes_left_button_double_click() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX pointer double click", 200, 120)
        .expect("native window");
    let window_id = window.window_id();
    let hwnd = window.native_handle().native_window();
    platform.dispatch_pending();
    while platform.next_event().is_some() {}

    unsafe {
        assert_ne!(
            PostMessageW(hwnd, WM_LBUTTONDBLCLK, 0, size_lparam(12, 16)),
            0
        );
        assert_ne!(PostMessageW(hwnd, WM_LBUTTONUP, 0, size_lparam(12, 16)), 0);
    }
    assert!(platform.dispatch_pending());

    let double_click = std::iter::from_fn(|| platform.next_event())
        .find(|event| event.type_ == UiEventType::PointerDoubleClick)
        .expect("double click event");
    assert_eq!(double_click.window_id, Some(window_id));
    assert!(matches!(
        double_click.payload,
        UiEventPayload::PointerButton(data) if data.btn == MouseButton::Left
    ));

    window.close().expect("close native window");
    assert!(platform.dispatch_pending());
}

#[test]
fn native_system_menu_operation_posts_caption_right_button_release() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX native system menu", 200, 120)
        .expect("native window");
    let hwnd = window.native_handle().native_window();
    platform.dispatch_pending();
    while platform.next_event().is_some() {}

    window.show_system_menu().expect("show system menu");
    let mut message = MSG::default();
    // SAFETY: `message` 在调用期间保持有效可写，HWND 由刚创建且仍存活的窗口提供。
    let found = unsafe {
        PeekMessageW(
            &mut message,
            Some(HWND(hwnd)),
            WM_NCRBUTTONUP,
            WM_NCRBUTTONUP,
            PM_REMOVE,
        )
    };
    assert!(
        found.as_bool(),
        "system menu operation must post WM_NCRBUTTONUP"
    );
    assert_eq!(message.wParam, WPARAM(HTCAPTION));

    window.close().expect("close native window");
    assert!(platform.dispatch_pending());
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

    let first_dpi = dpi_for_window(first_hwnd);
    let second_dpi = dpi_for_window(second_hwnd);
    unsafe {
        assert_ne!(
            PostMessageW(
                first_hwnd,
                WM_SIZE,
                SIZE_RESTORED,
                size_lparam(
                    logical_extent_to_physical(321, first_dpi) as u16,
                    logical_extent_to_physical(222, first_dpi) as u16,
                ),
            ),
            0
        );
        assert_ne!(
            PostMessageW(
                second_hwnd,
                WM_SIZE,
                SIZE_RESTORED,
                size_lparam(
                    logical_extent_to_physical(654, second_dpi) as u16,
                    logical_extent_to_physical(333, second_dpi) as u16,
                ),
            ),
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
fn native_window_keeps_logical_client_size_and_physical_drawable_size() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX Per-Monitor V2 extent", 321, 219)
        .expect("native window");
    let hwnd = window.native_handle().native_window();
    let dpi = dpi_for_window(hwnd);
    let awareness = unsafe { GetWindowDpiAwarenessContext(HWND(hwnd)) };
    assert!(bool::from(unsafe {
        AreDpiAwarenessContextsEqual(awareness, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
    }));
    let client = unsafe { query_client_rect(hwnd) }.expect("client rect");
    let physical = (client.right - client.left, client.bottom - client.top);
    let drawable = drawable_size(hwnd, 321, 219);

    assert_eq!(
        physical,
        (
            logical_extent_to_physical(321, dpi),
            logical_extent_to_physical(219, dpi),
        )
    );
    assert_eq!(
        (drawable.logical_width, drawable.logical_height),
        (321, 219)
    );
    assert_eq!((drawable.width, drawable.height), physical);
    assert_eq!(
        (window.properties().width(), window.properties().height()),
        (321, 219)
    );
    println!(
        "Windows Per-Monitor V2 extent: dpi={dpi} logical=321x219 physical={}x{}",
        physical.0, physical.1
    );

    window
        .properties_mut()
        .set_size(411, 277)
        .expect("resize logical client");
    assert!(platform.dispatch_pending());
    let resized_client = unsafe { query_client_rect(hwnd) }.expect("resized client rect");
    assert_eq!(
        (
            resized_client.right - resized_client.left,
            resized_client.bottom - resized_client.top,
        ),
        (
            logical_extent_to_physical(411, dpi),
            logical_extent_to_physical(277, dpi),
        )
    );
    assert_eq!(
        (window.properties().width(), window.properties().height()),
        (411, 277)
    );

    window.close().expect("close window");
}

#[test]
fn custom_title_bar_removes_caption_but_keeps_resize_frame_and_client_size() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX custom title bar", 419, 263)
        .expect("native window");
    let hwnd = window.native_handle().native_window();
    let dpi = dpi_for_window(hwnd);

    configure_custom_title_bar(window.as_mut(), 419, 263).expect("configure custom title bar");

    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    assert_eq!(style & WS_CAPTION, 0, "system caption must be removed");
    assert_ne!(
        style & WS_THICKFRAME,
        0,
        "native resize frame must remain available"
    );

    let client = unsafe { query_client_rect(hwnd) }.expect("custom title bar client rect");
    assert_eq!(
        (client.right - client.left, client.bottom - client.top),
        (
            logical_extent_to_physical(419, dpi),
            logical_extent_to_physical(263, dpi),
        )
    );
    assert_eq!(
        (window.properties().width(), window.properties().height()),
        (419, 263)
    );

    window.close().expect("close window");
}

#[test]
fn native_window_opacity_restores_original_layered_style() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX opacity restore", 200, 120)
        .expect("native window");
    let hwnd = window.native_handle().native_window();
    let original_ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;

    window
        .properties_mut()
        .set_window_opacity(0.4)
        .expect("set translucent opacity");
    let translucent_ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
    assert_ne!(
        translucent_ex_style & WS_EX_LAYERED,
        0,
        "translucent windows must enable WS_EX_LAYERED"
    );

    window
        .properties_mut()
        .set_window_opacity(1.0)
        .expect("restore opaque window");
    let restored_ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
    assert_eq!(
        restored_ex_style, original_ex_style,
        "restoring opacity must not leave UIX-owned layered styling behind"
    );

    window.close().expect("close window");
}

#[test]
fn native_window_centers_in_nearest_monitor_work_area() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX monitor work-area center", 360, 220)
        .expect("native window");
    let hwnd = window.native_handle().native_window();

    window.center_on_screen().expect("center native window");

    let mut rect = RECT::default();
    unsafe { GetWindowRect(HWND(hwnd), &mut rect) }.expect("centered window rect");
    let monitor = unsafe { MonitorFromWindowRaw(hwnd, MONITOR_DEFAULTTONEAREST) };
    assert!(!monitor.is_null(), "nearest monitor");
    let mut monitor_info = crate::native::backends::windows::bindings::MONITORINFO {
        cbSize: std::mem::size_of::<crate::native::backends::windows::bindings::MONITORINFO>()
            as u32,
        rcMonitor: crate::native::backends::windows::bindings::RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        rcWork: crate::native::backends::windows::bindings::RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        dwFlags: 0,
    };
    assert_ne!(unsafe { GetMonitorInfoRaw(monitor, &mut monitor_info) }, 0);

    assert!(
        ((rect.left + rect.right) - (monitor_info.rcWork.left + monitor_info.rcWork.right)).abs()
            <= 1,
        "window and work-area horizontal centers must match"
    );
    assert!(
        ((rect.top + rect.bottom) - (monitor_info.rcWork.top + monitor_info.rcWork.bottom)).abs()
            <= 1,
        "window and work-area vertical centers must match"
    );

    window.close().expect("close window");
}

#[test]
fn window_style_mutation_reports_invalid_handle_as_typed_error() {
    let error = set_window_long_checked(
        std::ptr::null_mut(),
        GWL_STYLE,
        0,
        "test SetWindowLongW failure",
    )
    .expect_err("invalid HWND must fail");

    assert_eq!(error.code(), crate::core::Errc::PlatformError);
    assert!(error.message().contains("test SetWindowLongW failure"));
    assert!(error.message().contains("Windows error"));
}

#[test]
fn native_dpi_change_applies_suggested_rect_and_routes_logical_resize() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX WM_DPICHANGED route", 320, 210)
        .expect("native window");
    let window_id = window.window_id();
    let hwnd = window.native_handle().native_window();
    assert!(platform.dispatch_pending());
    while platform.next_event().is_some() {}

    let mut original = RECT::default();
    unsafe { GetWindowRect(HWND(hwnd), &mut original) }.expect("original window rect");
    let suggested = RECT {
        left: original.left + 7,
        top: original.top + 11,
        right: original.right + 87,
        bottom: original.bottom + 71,
    };
    let dpi = dpi_for_window(hwnd) as usize;
    let dpi_wparam = dpi | (dpi << 16);
    unsafe {
        SendMessageW(
            HWND(hwnd),
            WM_DPICHANGED,
            Some(WPARAM(dpi_wparam)),
            Some(LPARAM((&suggested as *const RECT) as isize)),
        );
    }

    let mut applied = RECT::default();
    unsafe { GetWindowRect(HWND(hwnd), &mut applied) }.expect("applied window rect");
    assert_eq!(applied.left, suggested.left);
    assert_eq!(applied.top, suggested.top);
    assert_eq!(applied.right, suggested.right);
    assert_eq!(applied.bottom, suggested.bottom);

    platform.dispatch_pending();
    let client = unsafe { query_client_rect(hwnd) }.expect("client after DPI change");
    let expected = (
        physical_extent_to_logical(client.right - client.left, dpi as u32),
        physical_extent_to_logical(client.bottom - client.top, dpi as u32),
    );
    let resize = std::iter::from_fn(|| platform.next_event()).find_map(|event| {
        if event.window_id != Some(window_id) || event.type_ != UiEventType::WindowResize {
            return None;
        }
        let UiEventPayload::Resize(data) = event.payload else {
            return None;
        };
        Some((data.width, data.height))
    });
    assert_eq!(resize, Some(expected));
    assert_eq!(
        (window.properties().width(), window.properties().height()),
        expected
    );

    window.close().expect("close window");
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
