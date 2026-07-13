use crate::native::backends::windows::gdi_presenter::{clip_damage_rect, GdiPresenter};
use crate::native::traits::present::IPresenter;
use crate::tests::common::*;

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
