use crate::draw::backend::contract::RenderBackend;
use crate::draw::backend::cpu::*;
use crate::tests::common::*;

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
fn resize_normalizes_extent_and_preserves_surface_after_allocation_failure() {
    let mut backend = CpuBackend::new();
    backend.resize(8, 4).expect("initial resize");
    assert_eq!(backend.memory_usage(), 8 * 4 * 4);

    let error = backend
        .resize(i32::MAX, i32::MAX)
        .expect_err("oversized resize must fail");
    assert_eq!(error.code(), Errc::GraphicsOutOfMemory);
    assert_eq!((backend.width(), backend.height()), (8, 4));
    assert_eq!(backend.memory_usage(), 8 * 4 * 4);

    backend.resize(0, -1).expect("normalized resize");
    assert_eq!((backend.width(), backend.height()), (1, 1));
    assert_eq!(backend.memory_usage(), 4);
}

#[test]
fn encoded_picture_replaces_cpu_offscreen_pixels_and_begin_clears_stale_content() {
    use crate::draw::command::{EncodedPictureExecution, FrameEncoder, FrameRasterOp, FrameRect};

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

#[test]
fn offscreen_begin_resets_canvas_state_and_end_propagates_deferred_error() {
    use crate::draw::geometry::path::PathBuilder;
    use crate::draw::geometry::types::Transform;

    let mut backend = CpuBackend::new();
    let handle = backend.create_offscreen(4, 3).expect("Picture target");
    backend
        .try_begin_offscreen_paint(&handle)
        .expect("begin first Picture paint");
    {
        let canvas = backend.offscreen_canvas(&handle).expect("offscreen canvas");
        canvas.save();
        canvas.set_transform(Transform::translate(2.0, 1.0));
        canvas.push_clip(Rect::new(0.0, 0.0, 1.0, 1.0));
        canvas.set_opacity(0.5);
        canvas.push_clip_path(&PathBuilder::new().build());
    }
    let error = backend
        .try_end_offscreen_paint()
        .expect_err("end must surface an unflushed deferred error");
    assert_eq!(error.code(), Errc::NotImplemented);

    backend
        .try_begin_offscreen_paint(&handle)
        .expect("begin reused Picture paint");
    {
        let canvas = backend.offscreen_canvas(&handle).expect("reused canvas");
        canvas.restore();
        assert!(canvas.current_transform().is_identity());
        assert_eq!(canvas.offset(), (0.0, 0.0));
        assert_eq!(canvas.opacity(), 1.0);
        assert_eq!(canvas.current_clip(), Rect::new(0.0, 0.0, 4.0, 3.0));
        canvas.fill_rect(Rect::new(0.0, 0.0, 4.0, 3.0), Color::green(), None);
    }
    backend
        .try_flush_offscreen_paint(&handle)
        .expect("flush reused Picture paint");
    backend
        .try_end_offscreen_paint()
        .expect("end reused Picture paint");
    assert!(backend
        .copy_offscreen_pixels(&handle)
        .expect("reused pixels")
        .0
        .iter()
        .all(|pixel| *pixel == Color::green().premultiplied()));
}
