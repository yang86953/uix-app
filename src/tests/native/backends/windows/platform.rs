use crate::app::window_actions::configure_custom_title_bar;
use crate::native::backends::windows::consts::{
    GWL_EXSTYLE, GWL_STYLE, HTCAPTION, MONITOR_DEFAULTTONEAREST, SIZE_RESTORED, WM_CHAR,
    WM_DPICHANGED, WM_IME_ENDCOMPOSITION, WM_IME_STARTCOMPOSITION, WM_LBUTTONDBLCLK, WM_LBUTTONUP,
    WM_NCRBUTTONUP, WM_SETTINGCHANGE, WM_SIZE, WM_THEMECHANGED, WS_CAPTION, WS_EX_LAYERED,
    WS_MAXIMIZEBOX, WS_THICKFRAME,
};
use crate::native::backends::windows::display::WindowsDisplay;
use crate::native::backends::windows::dpi::{
    dpi_for_window, logical_extent_to_physical, outer_size_for_logical_client,
    physical_extent_to_logical,
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
    GetWindowRect, PeekMessageW, SendMessageW, SetWindowPos, MINMAXINFO, MSG, PM_REMOVE,
    SWP_NOSIZE, SWP_NOZORDER, WM_GETICON, WM_GETMINMAXINFO,
};

fn size_lparam(width: u16, height: u16) -> isize {
    (u32::from(width) | (u32::from(height) << 16)) as isize
}

fn write_test_icon() -> std::path::PathBuf {
    const WIDTH: u32 = 16;
    const HEIGHT: u32 = 16;
    const XOR_BYTES: u32 = WIDTH * HEIGHT * 4;
    const AND_BYTES: u32 = HEIGHT * 4;
    const IMAGE_BYTES: u32 = 40 + XOR_BYTES + AND_BYTES;

    let mut icon = Vec::with_capacity((22 + IMAGE_BYTES) as usize);
    icon.extend_from_slice(&0u16.to_le_bytes());
    icon.extend_from_slice(&1u16.to_le_bytes());
    icon.extend_from_slice(&1u16.to_le_bytes());
    icon.extend_from_slice(&[WIDTH as u8, HEIGHT as u8, 0, 0]);
    icon.extend_from_slice(&1u16.to_le_bytes());
    icon.extend_from_slice(&32u16.to_le_bytes());
    icon.extend_from_slice(&IMAGE_BYTES.to_le_bytes());
    icon.extend_from_slice(&22u32.to_le_bytes());
    icon.extend_from_slice(&40u32.to_le_bytes());
    icon.extend_from_slice(&(WIDTH as i32).to_le_bytes());
    icon.extend_from_slice(&((HEIGHT * 2) as i32).to_le_bytes());
    icon.extend_from_slice(&1u16.to_le_bytes());
    icon.extend_from_slice(&32u16.to_le_bytes());
    icon.extend_from_slice(&0u32.to_le_bytes());
    icon.extend_from_slice(&XOR_BYTES.to_le_bytes());
    icon.extend_from_slice(&0i32.to_le_bytes());
    icon.extend_from_slice(&0i32.to_le_bytes());
    icon.extend_from_slice(&0u32.to_le_bytes());
    icon.extend_from_slice(&0u32.to_le_bytes());
    for _ in 0..(WIDTH * HEIGHT) {
        icon.extend_from_slice(&[0x20, 0x80, 0xF0, 0xFF]);
    }
    icon.resize((22 + IMAGE_BYTES) as usize, 0);

    let path = std::env::temp_dir().join(format!("uix-window-icon-{}.ico", std::process::id()));
    std::fs::write(&path, icon).expect("write temporary ICO");
    path
}

const WS_EX_TOPMOST: u32 = 0x00000008;

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
fn native_window_icon_loads_both_sizes_and_preserves_them_on_failure() {
    let icon_path = write_test_icon();
    let missing_path = icon_path.with_file_name("uix-window-icon-missing.ico");
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX native icon", 200, 120)
        .expect("native window");
    let hwnd = HWND(window.native_handle().native_window());

    window
        .set_window_icon(icon_path.to_string_lossy().as_ref())
        .expect("load native window icon");
    let large = unsafe { SendMessageW(hwnd, WM_GETICON, Some(WPARAM(1)), Some(LPARAM(0))) };
    let small = unsafe { SendMessageW(hwnd, WM_GETICON, Some(WPARAM(0)), Some(LPARAM(0))) };
    assert_ne!(large.0, 0, "large HICON must be installed");
    assert_ne!(small.0, 0, "small HICON must be installed");

    let error = window
        .set_window_icon(missing_path.to_string_lossy().as_ref())
        .expect_err("missing icon must fail");
    assert_eq!(error.code(), Errc::PlatformError);
    assert_eq!(
        unsafe { SendMessageW(hwnd, WM_GETICON, Some(WPARAM(1)), Some(LPARAM(0))) },
        large,
        "failed replacement must preserve the installed large icon"
    );
    assert_eq!(
        unsafe { SendMessageW(hwnd, WM_GETICON, Some(WPARAM(0)), Some(LPARAM(0))) },
        small,
        "failed replacement must preserve the installed small icon"
    );

    window.close().expect("close native window");
    std::fs::remove_file(icon_path).expect("remove temporary ICO");
}

#[test]
fn native_window_flash_requests_attention_and_rejects_destroyed_handles() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX native flash", 200, 120)
        .expect("native window");

    window.flash_window().expect("request taskbar attention");
    window.close().expect("close native window");
    let error = window
        .flash_window()
        .expect_err("destroyed HWND must reject attention request");
    assert_eq!(error.code(), Errc::InvalidState);
}

#[test]
fn native_theme_messages_emit_one_app_wide_change_across_windows() {
    let mut platform = WindowsPlatform::new();
    let mut first = platform
        .create_window("UIX theme route A", 200, 120)
        .expect("first native window");
    let mut second = platform
        .create_window("UIX theme route B", 200, 120)
        .expect("second native window");
    platform.dispatch_pending();
    while platform.next_event().is_some() {}

    let actual_is_dark = WindowsDisplay::detect_os_theme();
    assert!(platform.route_system_theme_change(first.window_id(), !actual_is_dark));
    while platform.next_event().is_some() {}

    unsafe {
        SendMessageW(
            HWND(first.native_handle().native_window()),
            WM_THEMECHANGED,
            Some(WPARAM(0)),
            Some(LPARAM(0)),
        );
        SendMessageW(
            HWND(second.native_handle().native_window()),
            WM_SETTINGCHANGE,
            Some(WPARAM(0)),
            Some(LPARAM(0)),
        );
    }

    let events = std::iter::from_fn(|| platform.next_event())
        .filter(|event| event.type_ == UiEventType::ThemeChanged)
        .collect::<Vec<_>>();
    assert_eq!(
        events.len(),
        1,
        "theme broadcasts from multiple HWNDs must deduplicate"
    );
    assert_eq!(events[0].window_id, Some(first.window_id()));
    assert!(matches!(
        events[0].payload,
        UiEventPayload::ThemeChanged(ref data) if data.is_dark == actual_is_dark
    ));

    second.close().expect("close second window");
    first.close().expect("close first window");
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
fn native_move_tracks_outer_window_origin_instead_of_client_origin() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX outer position", 320, 200)
        .expect("native window");
    let hwnd = window.native_handle().native_window();

    unsafe { SetWindowPos(HWND(hwnd), None, 83, 97, 0, 0, SWP_NOSIZE | SWP_NOZORDER) }
        .expect("move native window outside property API");

    let mut rect = RECT::default();
    unsafe { GetWindowRect(HWND(hwnd), &mut rect) }.expect("moved window rect");
    assert_eq!((rect.left, rect.top), (83, 97));
    assert_eq!(window.properties().position(), Point::new(83.0, 97.0));

    window.close().expect("close window");
}

#[test]
fn native_minimum_and_maximum_sizes_use_logical_client_extents() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX track constraints", 480, 320)
        .expect("native window");
    let hwnd = window.native_handle().native_window();
    window
        .properties_mut()
        .set_minimum_size(300, 180)
        .expect("set minimum logical client size");
    window
        .properties_mut()
        .set_maximum_size(700, 480)
        .expect("set maximum logical client size");

    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
    let dpi = dpi_for_window(hwnd);
    let expected_min = outer_size_for_logical_client(300, 180, style, ex_style, dpi)
        .expect("minimum outer track size");
    let expected_max = outer_size_for_logical_client(700, 480, style, ex_style, dpi)
        .expect("maximum outer track size");
    let mut limits = MINMAXINFO::default();
    unsafe {
        SendMessageW(
            HWND(hwnd),
            WM_GETMINMAXINFO,
            Some(WPARAM(0)),
            Some(LPARAM((&mut limits as *mut MINMAXINFO) as isize)),
        );
    }

    assert_eq!(
        (limits.ptMinTrackSize.x, limits.ptMinTrackSize.y),
        expected_min
    );
    assert_eq!(
        (limits.ptMaxTrackSize.x, limits.ptMaxTrackSize.y),
        expected_max
    );

    window
        .properties_mut()
        .set_fullscreen(true)
        .expect("enter fullscreen with constraints");
    let mut fullscreen_limits = MINMAXINFO::default();
    unsafe {
        SendMessageW(
            HWND(hwnd),
            WM_GETMINMAXINFO,
            Some(WPARAM(0)),
            Some(LPARAM((&mut fullscreen_limits as *mut MINMAXINFO) as isize)),
        );
    }
    assert_ne!(
        (
            fullscreen_limits.ptMinTrackSize.x,
            fullscreen_limits.ptMinTrackSize.y,
        ),
        expected_min,
        "fullscreen must not inherit the normal-window minimum"
    );
    assert_ne!(
        (
            fullscreen_limits.ptMaxTrackSize.x,
            fullscreen_limits.ptMaxTrackSize.y,
        ),
        expected_max,
        "fullscreen must not inherit the normal-window maximum"
    );
    window
        .properties_mut()
        .set_fullscreen(false)
        .expect("leave fullscreen with constraints");

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
    let mut outer = RECT::default();
    assert!(
        unsafe { GetWindowRect(HWND(hwnd), &mut outer) }.is_ok(),
        "GetWindowRect must succeed"
    );
    let client_w = client.right - client.left;
    let client_h = client.bottom - client.top;
    let outer_w = outer.right - outer.left;
    let outer_h = outer.bottom - outer.top;
    assert_eq!(
        (client_w, client_h),
        (
            logical_extent_to_physical(419, dpi),
            logical_extent_to_physical(263, dpi),
        )
    );
    assert_eq!(
        (client_w, client_h),
        (outer_w, outer_h),
        "extended client must fill the outer window so content is not inset by THICKFRAME"
    );
    assert_eq!(
        (window.properties().width(), window.properties().height()),
        (419, 263)
    );

    window.close().expect("close window");
}

#[test]
fn native_resizable_toggle_controls_resize_frame_and_maximize_entry() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX resizable toggle", 320, 180)
        .expect("native window");
    let hwnd = window.native_handle().native_window();
    let resize_style = WS_THICKFRAME | WS_MAXIMIZEBOX;

    let initial_style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    assert_eq!(
        initial_style & resize_style,
        resize_style,
        "resizable windows must expose both resize paths"
    );

    window
        .properties_mut()
        .set_resizable(false)
        .expect("disable native resize paths");
    let fixed_style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    assert_eq!(
        fixed_style & resize_style,
        0,
        "fixed-size windows must reject both border drag and maximize resize"
    );

    window
        .properties_mut()
        .set_resizable(true)
        .expect("restore native resize paths");
    let restored_style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    assert_eq!(
        restored_style & resize_style,
        resize_style,
        "re-enabling resize must restore border drag and maximize"
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
fn native_fullscreen_restores_style_placement_and_z_order() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX fullscreen restore", 419, 263)
        .expect("native window");
    let hwnd = window.native_handle().native_window();
    configure_custom_title_bar(window.as_mut(), 419, 263).expect("configure custom title bar");
    window
        .properties_mut()
        .set_position(73, 91)
        .expect("position window before fullscreen");

    let mut original_rect = RECT::default();
    unsafe { GetWindowRect(HWND(hwnd), &mut original_rect) }.expect("original window rect");
    let original_style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    let original_ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
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

    window
        .properties_mut()
        .set_fullscreen(true)
        .expect("enter fullscreen");
    let mut fullscreen_rect = RECT::default();
    unsafe { GetWindowRect(HWND(hwnd), &mut fullscreen_rect) }.expect("fullscreen window rect");
    assert_eq!(fullscreen_rect.left, monitor_info.rcMonitor.left);
    assert_eq!(fullscreen_rect.top, monitor_info.rcMonitor.top);
    assert_eq!(fullscreen_rect.right, monitor_info.rcMonitor.right);
    assert_eq!(fullscreen_rect.bottom, monitor_info.rcMonitor.bottom);
    let fullscreen_ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
    assert_eq!(
        fullscreen_ex_style & WS_EX_TOPMOST,
        original_ex_style & WS_EX_TOPMOST,
        "fullscreen must preserve always-on-top state"
    );

    window
        .properties_mut()
        .set_fullscreen(false)
        .expect("exit fullscreen");
    let mut restored_rect = RECT::default();
    unsafe { GetWindowRect(HWND(hwnd), &mut restored_rect) }.expect("restored window rect");
    assert_eq!(
        unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32,
        original_style
    );
    assert_eq!(restored_rect, original_rect);

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
