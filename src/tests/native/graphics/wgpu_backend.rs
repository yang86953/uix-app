use crate::native::graphics::wgpu_backend::draw_stream::grow_zeroed;
use crate::native::graphics::wgpu_backend::{
    choose_surface_alpha_mode, device_limits_for_adapter, ensure_surface_extent,
    logical_draw_viewport,
};

#[test]
fn glyph_staging_never_shrinks_when_shorter_glyphs_share_a_row() {
    let mut staging = Vec::new();
    grow_zeroed(&mut staging, 56 * 1024);
    grow_zeroed(&mut staging, 49 * 1024);

    assert_eq!(staging.len(), 56 * 1024);
}

#[test]
fn surface_alpha_prefers_opaque_over_premultiplied() {
    let modes = [
        wgpu::CompositeAlphaMode::PreMultiplied,
        wgpu::CompositeAlphaMode::Opaque,
        wgpu::CompositeAlphaMode::PostMultiplied,
    ];
    assert_eq!(
        choose_surface_alpha_mode(&modes),
        Some(wgpu::CompositeAlphaMode::Opaque)
    );
    assert_eq!(
        choose_surface_alpha_mode(&[wgpu::CompositeAlphaMode::PreMultiplied]),
        Some(wgpu::CompositeAlphaMode::PreMultiplied)
    );
    assert!(choose_surface_alpha_mode(&[]).is_none());
}

#[test]
fn swapchain_picture_blit_viewport_stays_logical_under_dpr() {
    assert_eq!(logical_draw_viewport(None, 1200, 800), (1200.0, 800.0));
    assert_eq!(
        logical_draw_viewport(Some((320, 240)), 1200, 800),
        (320.0, 240.0)
    );
}

#[test]
fn device_limits_raise_texture_dimension_for_desktop_swapchain() {
    let mut adapter = wgpu::Limits::downlevel_defaults();
    assert_eq!(adapter.max_texture_dimension_2d, 2048);
    adapter.max_texture_dimension_2d = 16384;
    let limits = device_limits_for_adapter(&adapter);
    assert_eq!(limits.max_texture_dimension_2d, 16384);
    assert!(
        ensure_surface_extent(2558, 1438, limits.max_texture_dimension_2d).is_ok(),
        "1440p maximize must fit raised texture limit"
    );
}

#[test]
fn surface_extent_rejects_sizes_above_device_limit() {
    let err = ensure_surface_extent(2558, 1438, 2048).expect_err("must reject 1440p on 2048 limit");
    assert_eq!(err.code(), crate::core::Errc::GraphicsOutOfMemory);
}
