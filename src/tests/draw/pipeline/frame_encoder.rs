use crate::tests::common::*;
use crate::draw::pipeline::frame_encoder::*;

fn rect(x: i32, y: i32, width: i32, height: i32, color: Color) -> FrameRasterOp {
    FrameRasterOp::FillRect {
        rect: FrameRect::new(x, y, width, height),
        color,
    }
}

#[test]
fn clear_picture_native_draw_follows_painter_order() {
    let mut encoder = FrameEncoder::new(4, 4).unwrap();
    encoder.clear(Color::black());
    encoder.blit_picture(
        FrameImage::solid(4, 4, Color::blue()).unwrap(),
        FrameRect::new(0, 0, 4, 4),
        FrameRect::new(0, 0, 4, 4),
    );
    encoder.native(rect(1, 1, 2, 2, Color::red()));

    let frame = encoder.render_reference();
    assert_eq!(frame.pixel(0, 0), Some(Color::blue().to_rgba()));
    assert_eq!(frame.pixel(1, 1), Some(Color::red().to_rgba()));
    assert_eq!(frame.pixel(2, 2), Some(Color::red().to_rgba()));
}

#[test]
fn native_draw_picture_blit_follows_painter_order() {
    let mut encoder = FrameEncoder::new(4, 4).unwrap();
    encoder.clear(Color::black());
    encoder.native(rect(1, 1, 2, 2, Color::red()));
    encoder.blit_picture(
        FrameImage::solid(4, 4, Color::blue()).unwrap(),
        FrameRect::new(0, 0, 4, 4),
        FrameRect::new(0, 0, 4, 4),
    );

    let frame = encoder.render_reference();
    assert_eq!(frame.pixel(1, 1), Some(Color::blue().to_rgba()));
    assert_eq!(frame.pixel(3, 3), Some(Color::blue().to_rgba()));
}

#[test]
fn native_cpu_segment_native_preserves_the_full_command_order() {
    let mut encoder = FrameEncoder::new(5, 1).unwrap();
    encoder.clear(Color::black());
    encoder.native(rect(0, 0, 5, 1, Color::red()));
    encoder.cpu_segment([rect(1, 0, 3, 1, Color::green())]);
    encoder.native(rect(2, 0, 1, 1, Color::blue()));

    let frame = encoder.render_reference();
    assert_eq!(
        frame.pixels(),
        &[
            Color::red().to_rgba(),
            Color::green().to_rgba(),
            Color::blue().to_rgba(),
            Color::green().to_rgba(),
            Color::red().to_rgba(),
        ]
    );
}

#[test]
fn cpu_image_segment_preserves_its_source_crop_and_destination() {
    let mut encoder = FrameEncoder::new(5, 2).unwrap();
    encoder.clear(Color::black());
    let image = FrameImage::new(
        3,
        2,
        vec![
            Color::red().premultiplied(),
            Color::green().premultiplied(),
            Color::blue().premultiplied(),
            Color::white().premultiplied(),
            Color::transparent().premultiplied(),
            Color::red().premultiplied(),
        ],
    )
    .unwrap();
    encoder.cpu_image_segment(
        image,
        FrameRect::new(1, 0, 1, 2),
        FrameRect::new(2, 0, 2, 2),
    );

    let frame = encoder.render_reference();
    assert_eq!(frame.pixel(1, 0), Some(Color::black().premultiplied()));
    assert_eq!(frame.pixel(2, 0), Some(Color::green().premultiplied()));
    assert_eq!(frame.pixel(3, 1), Some(Color::black().premultiplied()));
}

struct RecordingPresenter {
    calls: usize,
    frame: Option<ReferenceFrame>,
}

impl FramePresenter for RecordingPresenter {
    type Error = ();

    fn present(&mut self, frame: &ReferenceFrame) -> Result<PresentOutcome, Self::Error> {
        self.calls += 1;
        self.frame = Some(frame.clone());
        Ok(PresentOutcome::Presented)
    }
}

#[test]
fn consuming_encoder_submits_exactly_one_frame() {
    let mut encoder = FrameEncoder::new(2, 1).unwrap();
    encoder.clear(Color::black());
    encoder.native(rect(0, 0, 1, 1, Color::white()));
    let mut presenter = RecordingPresenter {
        calls: 0,
        frame: None,
    };

    let outcome = encoder.present(&mut presenter).unwrap();

    assert_eq!(outcome, PresentOutcome::Presented);
    assert_eq!(presenter.calls, 1);
    assert_eq!(
        presenter.frame.unwrap().pixels(),
        &[Color::white().to_rgba(), Color::black().to_rgba()]
    );
}

#[test]
fn reference_executor_matches_cpu_rasterizer_for_transparent_rect_layers() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
    use crate::draw::traits::Canvas2D;

    let clear = Color::from_rgba(16, 32, 64, 96);
    let first = Color::from_rgba(255, 0, 0, 128);
    let second = Color::from_rgba(0, 96, 255, 144);
    let mut encoder = FrameEncoder::new(4, 3).unwrap();
    encoder.clear(clear);
    encoder.native(rect(-1, 0, 3, 3, first));
    encoder.cpu_segment([rect(1, 1, 4, 3, second)]);

    let mut cpu = SharedRasterizer::new(PixelSurface::new(4, 3));
    cpu.surface_mut().set_clear_color(clear);
    cpu.surface_mut().clear_all();
    cpu.fill_rect(Rect::new(-1.0, 0.0, 3.0, 3.0), first, None);
    cpu.fill_rect(Rect::new(1.0, 1.0, 4.0, 3.0), second, None);

    assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());
}

#[test]
fn additive_fill_matches_cpu_rasterizer_destination_blend() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
    use crate::draw::primitives::types::BlendMode;
    use crate::draw::traits::Canvas2D;

    let base = Color::from_rgba(40, 80, 120, 200);
    let add = Color::from_rgba(30, 40, 50, 100);
    let mut encoder = FrameEncoder::new(3, 2).unwrap();
    encoder.clear(base);
    encoder.native(FrameRasterOp::FillRectAdditive {
        rect: FrameRect::new(1, 0, 1, 2),
        color: add,
    });

    let mut cpu = SharedRasterizer::new(PixelSurface::new(3, 2));
    cpu.surface_mut().set_clear_color(base);
    cpu.surface_mut().clear_all();
    cpu.set_blend_mode(BlendMode::Additive);
    cpu.fill_rect(Rect::new(1.0, 0.0, 1.0, 2.0), add, None);

    assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());
}

#[test]
fn scroll_copy_matches_cpu_rasterizer_scroll_region() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
    use crate::draw::traits::Canvas2D;

    let mut encoder = FrameEncoder::new(4, 3).unwrap();
    encoder.clear(Color::black());
    encoder.native(rect(0, 0, 2, 2, Color::red()));
    encoder.native(rect(2, 1, 2, 2, Color::blue()));
    encoder.native(FrameRasterOp::ScrollCopy {
        viewport: FrameRect::new(0, 0, 4, 3),
        dx: 0,
        dy: 1,
    });

    let mut cpu = SharedRasterizer::new(PixelSurface::new(4, 3));
    cpu.surface_mut().set_clear_color(Color::black());
    cpu.surface_mut().clear_all();
    cpu.fill_rect(Rect::new(0.0, 0.0, 2.0, 2.0), Color::red(), None);
    cpu.fill_rect(Rect::new(2.0, 1.0, 2.0, 2.0), Color::blue(), None);
    cpu.scroll_region(Rect::new(0.0, 0.0, 4.0, 3.0), 0.0, 1.0);

    assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());
}

#[test]
fn execute_into_pixels_without_clear_retains_undamaged_pixels() {
    let mut pixels = vec![Color::blue().premultiplied(); 4 * 4];
    let mut encoder = FrameEncoder::new(4, 4).unwrap();
    encoder.native(rect(1, 1, 2, 2, Color::red()));
    encoder.execute_into_pixels(&mut pixels);

    assert_eq!(pixels[0], Color::blue().premultiplied());
    assert_eq!(pixels[5], Color::red().premultiplied());
    assert_eq!(pixels[15], Color::blue().premultiplied());
}
