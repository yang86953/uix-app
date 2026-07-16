use crate::native::backends::windows::cursor::client_area_screen_rect;
use crate::native::backends::windows::platform::WindowsPlatform;
use crate::native::traits::IWindowManager;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

#[test]
fn cursor_confinement_rect_uses_client_area_in_screen_coordinates() {
    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX cursor client rect", 320, 200)
        .expect("native window");
    let hwnd = HWND(window.native_handle().native_window());

    let mut client = RECT::default();
    unsafe { GetClientRect(hwnd, &mut client) }.expect("query client rect");
    let mut top_left = POINT {
        x: client.left,
        y: client.top,
    };
    let mut bottom_right = POINT {
        x: client.right,
        y: client.bottom,
    };
    assert!(unsafe { ClientToScreen(hwnd, &mut top_left) }.as_bool());
    assert!(unsafe { ClientToScreen(hwnd, &mut bottom_right) }.as_bool());

    let actual = client_area_screen_rect(hwnd.0).expect("client screen rect");
    assert_eq!((actual.left, actual.top), (top_left.x, top_left.y));
    assert_eq!(
        (actual.right, actual.bottom),
        (bottom_right.x, bottom_right.y)
    );
    window.close().expect("close native window");
}

#[test]
fn cursor_confinement_rect_rejects_a_null_window() {
    assert!(client_area_screen_rect(std::ptr::null_mut()).is_none());
}
