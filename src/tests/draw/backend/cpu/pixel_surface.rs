use crate::draw::backend::cpu::pixel_surface::*;
use crate::tests::common::*;

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

#[test]
fn oversized_surface_returns_typed_out_of_memory_without_allocating() {
    let error = match PixelSurface::try_new(i32::MAX, i32::MAX) {
        Ok(_) => panic!("oversized surface must be rejected"),
        Err(error) => error,
    };

    assert_eq!(error.code(), Errc::GraphicsOutOfMemory);
}

#[test]
fn extreme_clear_and_copy_coordinates_are_clipped_without_overflow() {
    let mut surface = PixelSurface::new(3, 2);
    surface.pixels_mut().copy_from_slice(&[1, 2, 3, 4, 5, 6]);

    surface.clear_rect_raw(i32::MAX, i32::MAX, i32::MAX, i32::MAX);
    assert_eq!(surface.pixels(), [1, 2, 3, 4, 5, 6]);

    surface.copy_region(
        Rect::new(i32::MAX as f32, 0.0, i32::MAX as f32, 1.0),
        i32::MIN,
        i32::MAX,
    );
    assert_eq!(surface.pixels(), [1, 2, 3, 4, 5, 6]);

    surface.set_clear_color(Color::red());
    surface.clear_rect_raw(-1, 0, i32::MAX, 1);
    assert!(surface.pixels()[..3]
        .iter()
        .all(|&pixel| pixel == Color::red().premultiplied()));
}
