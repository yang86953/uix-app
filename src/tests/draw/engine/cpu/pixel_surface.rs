use crate::tests::common::*;
use crate::draw::traits::RenderingBackend;
use crate::draw::engine::cpu::pixel_surface::*;

#[test]
fn soft_surface_clears_to_transparent_not_opaque_black() {
    let mut surface = PixelSurface::new(4, 4);
    // Seed opaque pixels, then clear — soft blit must not wipe GPU-native draws.
    surface.pixels_mut().fill(0xFF00_00FF);
    surface.clear_all();
    assert_eq!(surface.clear_color(), Color::transparent());
    assert!(
        surface.pixels().iter().all(|&p| p == 0x0000_0000),
        "clear_all must write transparent (A=0), got {:?}",
        surface.pixels().first()
    );
    surface.clear_rect_raw(1, 1, 2, 2);
    let idx = 5usize;
    assert_eq!(surface.pixels()[idx], 0x0000_0000);
}

#[test]
fn translucent_clear_color_is_written_premultiplied() {
    let mut surface = PixelSurface::new(2, 1);
    let clear = Color::from_rgba(240, 120, 60, 128);
    surface.set_clear_color(clear);
    surface.clear_all();

    assert_eq!(
        surface.pixels(),
        [clear.premultiplied(), clear.premultiplied()]
    );
}
