use crate::tests::common::*;
use crate::draw::backend::offscreen_pool::CpuOffscreenPool;
use crate::draw::backend::traits::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::pipeline::{ EncodedFrameExecution, EncodedPictureExecution };
use crate::draw::rasterizer::image::blit_image;
use crate::draw::traits::Canvas2D;
use crate::draw::backend::cpu::*;

#[test]
fn create_offscreen_reuses_destroyed_ids() {
    let mut backend = CpuBackend::new();
    backend.resize(64, 64).expect("resize");
    let a = backend.create_offscreen(16, 16).expect("a");
    let b = backend.create_offscreen(16, 16).expect("b");
    assert_ne!(a.0, b.0);
    backend.destroy_offscreen(a);
    let c = backend.create_offscreen(8, 8).expect("c");
    assert_eq!(c.0, a.0, "destroyed id must be reused");
    assert_eq!(
        backend.offscreens.slot_len(),
        2,
        "slot vec must not grow on reuse"
    );
    backend.destroy_offscreen(b);
    backend.destroy_offscreen(c);
}

#[test]
fn encoded_picture_replaces_cpu_offscreen_pixels_and_begin_clears_stale_content() {
    use crate::draw::pipeline::{
        EncodedPictureExecution, FrameEncoder, FrameRasterOp, FrameRect,
    };

    let mut backend = CpuBackend::new();
    let handle = backend.create_offscreen(4, 3).expect("Picture target");
    {
        let canvas = backend.offscreen_canvas(&handle).expect("offscreen canvas");
        canvas.fill_rect(
            Rect::new(0.0, 0.0, 4.0, 3.0),
            Color::from_rgb(220, 10, 10),
            None,
        );
    }
    backend
        .try_begin_offscreen_paint(&handle)
        .expect("replace clear");
    assert!(backend
        .copy_offscreen_pixels(&handle)
        .expect("cleared pixels")
        .0
        .iter()
        .all(|&pixel| pixel == Color::transparent().premultiplied()));

    let mut encoder = FrameEncoder::new(4, 3).expect("encoder");
    encoder.clear(Color::transparent());
    encoder
        .cpu_segment([FrameRasterOp::FillRect {
            rect: FrameRect::new(1, 1, 2, 1),
            color: Color::from_rgba(20, 40, 200, 128),
        }])
        .unwrap();
    assert_eq!(
        backend
            .try_execute_encoded_picture(&handle, &encoder)
            .expect("FrameEncoder execution"),
        EncodedPictureExecution::Executed
    );
    let pixels = backend
        .copy_offscreen_pixels(&handle)
        .expect("encoded pixels")
        .0;
    assert_eq!(
        pixels[1 + 4],
        Color::from_rgba(20, 40, 200, 128).premultiplied()
    );
    assert_eq!(pixels[0], Color::transparent().premultiplied());
}
