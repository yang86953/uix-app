use crate::native::backends::windows::ffi::{GetDC, ReleaseDC};
use crate::native::backends::windows::gdi_presenter::{
    clip_damage_rect, scale_damage_rect_to_target, GdiPresenter,
};
use crate::native::graphics::platform::windows::query_client_rect;
use crate::native::traits::present::IPresenter;
use crate::tests::common::*;

#[link(name = "gdi32")]
extern "system" {
    fn GetPixel(hdc: *mut std::ffi::c_void, x: i32, y: i32) -> u32;
}

fn window_pixel(hwnd: *mut std::ffi::c_void, x: i32, y: i32) -> u32 {
    unsafe {
        let hdc = GetDC(hwnd);
        assert!(!hdc.is_null(), "GetDC must expose the presented client");
        let color = GetPixel(hdc, x, y);
        assert_ne!(ReleaseDC(hwnd, hdc), 0, "ReleaseDC must succeed");
        color
    }
}

#[test]
fn damage_padding_is_clipped_to_the_gdi_surface() {
    assert_eq!(
        clip_damage_rect(0, 0, 1202, 802, 1200, 800),
        Some((0, 0, 1200, 800))
    );
}

#[test]
fn negative_and_outside_damage_is_clipped_or_dropped() {
    assert_eq!(
        clip_damage_rect(-2, -3, 10, 11, 120, 80),
        Some((0, 0, 8, 8))
    );
    assert_eq!(clip_damage_rect(121, 0, 4, 4, 120, 80), None);
}

#[test]
fn logical_damage_scales_outward_to_physical_target_pixels() {
    assert_eq!(
        scale_damage_rect_to_target(1, 2, 3, 4, 800, 600, 1200, 900),
        Some((1, 3, 5, 6))
    );
    assert_eq!(
        scale_damage_rect_to_target(-2, -2, 4, 4, 2, 2, 5, 5),
        Some((0, 0, 5, 5))
    );
    assert_eq!(
        scale_damage_rect_to_target(0, 0, 800, 600, 800, 600, 1200, 900),
        Some((0, 0, 1200, 900))
    );
}

#[test]
fn gdi_stretches_full_and_partial_logical_frames_to_the_client() {
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("GDI logical to physical stretch", 20, 12)
        .expect("window");
    window.show().expect("show window");
    assert!(platform.event_loop().poll_event(&|_| true));
    let hwnd = window.native_surface_ptr();
    let client = unsafe { query_client_rect(hwnd) }.expect("client rect");
    let target_width = client.right - client.left;
    let target_height = client.bottom - client.top;
    assert!(target_width >= 2 && target_height >= 2);

    let mut presenter = unsafe { GdiPresenter::new(hwnd, 2, 2) }.expect("presenter");
    let initial = [0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0xffff_ffff];
    presenter
        .present(&initial, 2, 2, PresentDamage::Full)
        .expect("stretch full frame");

    let left = (target_width / 4).max(0);
    let right = (target_width * 3 / 4).min(target_width - 1);
    let top = (target_height / 4).max(0);
    let bottom = (target_height * 3 / 4).min(target_height - 1);
    assert_eq!(window_pixel(hwnd, left, top), 0x0000_00ff);
    assert_eq!(window_pixel(hwnd, right, top), 0x0000_ff00);
    assert_eq!(window_pixel(hwnd, left, bottom), 0x00ff_0000);
    assert_eq!(window_pixel(hwnd, right, bottom), 0x00ff_ffff);

    let partial = [0xffff_ff00, 0xff00_ff00, 0xff00_00ff, 0xffff_ffff];
    presenter
        .present(&partial, 2, 2, PresentDamage::Partial(vec![(0, 0, 1, 1)]))
        .expect("stretch partial frame");
    assert_eq!(window_pixel(hwnd, left, top), 0x0000_ffff);
    assert_eq!(window_pixel(hwnd, right, top), 0x0000_ff00);
    assert_eq!(window_pixel(hwnd, left, bottom), 0x00ff_0000);
    assert_eq!(window_pixel(hwnd, right, bottom), 0x00ff_ffff);

    let before_resize = presenter.present_surface(2, 2, 1.0);
    window
        .properties_mut()
        .set_size(28, 18)
        .expect("resize target client");
    let after_resize = presenter.present_surface(2, 2, 1.0);
    assert_ne!(before_resize.generation, after_resize.generation);
    window.close().expect("close window");
}

#[test]
fn present_propagates_rejected_resize_instead_of_using_the_old_dib() {
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("GDI resize failure", 16, 16)
        .expect("window");
    let mut presenter =
        unsafe { GdiPresenter::new(window.native_surface_ptr(), 16, 16) }.expect("presenter");
    assert_eq!(
        presenter.present_coherency(),
        PresentCoherency::RetainedBuffer
    );

    let error = presenter
        .present(&[0], 0, 1, PresentDamage::Full)
        .expect_err("invalid replacement extent must fail");
    assert_eq!(error.code(), Errc::InvalidArgument);
    window.close().expect("close window");
}

#[test]
fn present_reports_a_destroyed_window_instead_of_claiming_success() {
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("GDI destroyed window", 16, 16)
        .expect("window");
    let mut presenter =
        unsafe { GdiPresenter::new(window.native_surface_ptr(), 16, 16) }.expect("presenter");
    window.close().expect("close window");

    let error = presenter
        .present(&[0; 16 * 16], 16, 16, PresentDamage::Full)
        .expect_err("a destroyed window must reject the final GDI submit");
    assert_eq!(error.code(), Errc::PlatformError);
    assert!(error.message().contains("GetClientRect"));
}
