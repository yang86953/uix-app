use crate::core::Rect;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::primitives::color::Color;
use crate::draw::traits::Canvas2D;

use super::SharedRasterizer;

#[test]
fn clip_rect_follows_canvas_offset() {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(24, 12));

    canvas.set_offset(10.0, 4.0);
    Canvas2D::push_clip(&mut canvas, Rect::new(0.0, 0.0, 4.0, 3.0));
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 8.0, 6.0),
        Color::from_rgb(255, 0, 0),
        None,
    );
    canvas.pop_clip();

    let pixels = canvas.surface().pixels();
    let width = canvas.surface().width() as usize;
    let at = |x: usize, y: usize| pixels[y * width + x];
    assert_ne!(at(10, 4), 0, "translated clip must keep translated content");
    assert_ne!(
        at(13, 6),
        0,
        "translated clip must cover its lower-right edge"
    );
    assert_eq!(at(14, 6), 0, "clip width must still be enforced");
    assert_eq!(at(13, 7), 0, "clip height must still be enforced");
}
