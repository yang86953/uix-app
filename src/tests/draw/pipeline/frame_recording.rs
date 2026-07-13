use crate::tests::common::*;
use crate::draw::backend::{CpuBackend, RenderBackend};
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::pipeline::{ EncodedFrameExecution, EncodedPictureExecution, FrameEncoderError, FrameImage, FrameRasterOp };
use crate::draw::primitives::types::{ BlendMode, GradientDirection, Radius };
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use crate::draw::pipeline::frame_recording::*;

#[test]
fn recording_canvas_emits_native_cpu_and_picture_commands_in_painter_order() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(12, 8).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine
        .canvas_2d()
        .fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::red(), None);
    engine.canvas_2d().fill_rect(
        Rect::new(4.0, 1.0, 2.0, 2.0),
        Color::from_rgba(0, 120, 255, 128),
        Some(Radius::uniform(1.0)),
    );
    let picture = engine.create_offscreen(2, 2).expect("Picture");
    engine
        .try_begin_offscreen_paint(&picture)
        .expect("begin Picture");
    engine
        .offscreen_canvas(&picture)
        .expect("Picture canvas")
        .fill_rect(Rect::new(0.0, 0.0, 2.0, 2.0), Color::green(), None);
    engine
        .try_end_offscreen_paint()
        .expect("end Picture target");
    engine
        .try_blit_offscreen_src(
            &picture,
            Rect::new(0.0, 0.0, 2.0, 2.0),
            Rect::new(8.0, 4.0, 2.0, 2.0),
        )
        .expect("record Picture blit");
    engine
        .canvas_2d()
        .fill_rect(Rect::new(9.0, 1.0, 2.0, 2.0), Color::blue(), None);

    let encoder = engine.finish_recording().expect("finish recorder");
    assert!(matches!(
        encoder.commands()[0],
        crate::draw::pipeline::FrameCommand::Clear { .. }
    ));
    assert!(matches!(
        encoder.commands()[1],
        crate::draw::pipeline::FrameCommand::Native { .. }
    ));
    assert!(matches!(
        encoder.commands()[2],
        crate::draw::pipeline::FrameCommand::CpuSegment { .. }
    ));
    assert!(matches!(
        encoder.commands()[3],
        crate::draw::pipeline::FrameCommand::PictureBlit { .. }
    ));
    assert!(matches!(
        encoder.commands()[4],
        crate::draw::pipeline::FrameCommand::Native { .. }
    ));
}

#[test]
fn recording_canvas_retains_compact_cpu_segment_tiles() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(1200, 800).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    let color = Color::from_rgba(20, 40, 60, 128);
    engine
        .canvas_2d()
        .fill_rect(Rect::new(701.0, 503.0, 3.0, 2.0), color, None);

    let encoder = engine.finish_recording().expect("finish recorder");
    let cpu_segment = encoder
        .commands()
        .iter()
        .find_map(|command| match command {
            crate::draw::pipeline::FrameCommand::CpuSegment { image, src, dst } => {
                Some((image, src, dst))
            }
            _ => None,
        })
        .expect("CPU segment");

    assert_eq!((cpu_segment.0.width(), cpu_segment.0.height()), (3, 2));
    assert_eq!(cpu_segment.0.pixels().len(), 6);
    assert_eq!(*cpu_segment.1, FrameRect::new(0, 0, 3, 2));
    assert_eq!(*cpu_segment.2, FrameRect::new(701, 503, 3, 2));
    let reference = encoder.render_reference();
    assert_eq!(
        reference.pixel(700, 503),
        Some(Color::transparent().premultiplied())
    );
    assert_eq!(reference.pixel(701, 503), Some(color.premultiplied()));
    assert_eq!(reference.pixel(703, 504), Some(color.premultiplied()));
    assert_eq!(
        reference.pixel(704, 504),
        Some(Color::transparent().premultiplied())
    );
}

#[test]
fn additive_axis_aligned_fill_records_native_additive_op() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(2, 2).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine.canvas_2d().set_blend_mode(BlendMode::Additive);
    engine
        .canvas_2d()
        .fill_rect(Rect::new(0.0, 0.0, 1.0, 1.0), Color::red(), None);

    let encoder = engine
        .finish_recording()
        .expect("Additive axis-aligned fill must record as Native IR");
    assert!(encoder.commands().iter().any(|command| {
        matches!(
            command,
            crate::draw::pipeline::FrameCommand::Native {
                operation: FrameRasterOp::FillRectAdditive { .. }
            }
        )
    }));
}

#[test]
fn additive_rounded_fill_still_fails_instead_of_cpu_segment_approximation() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(2, 2).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine.canvas_2d().set_blend_mode(BlendMode::Additive);
    engine.canvas_2d().fill_rect(
        Rect::new(0.0, 0.0, 1.0, 1.0),
        Color::red(),
        Some(Radius::uniform(1.0)),
    );

    let error = engine
        .finish_recording()
        .expect_err("Additive rounded fill must not become a source-over CPU segment");
    assert_eq!(error.code(), Errc::NotImplemented);
}

#[test]
fn scroll_region_records_native_scroll_copy() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(8, 6).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine
        .canvas_2d()
        .fill_rect(Rect::new(0.0, 0.0, 4.0, 4.0), Color::red(), None);
    engine
        .canvas_2d()
        .scroll_region(Rect::new(0.0, 0.0, 8.0, 6.0), 0.0, 2.0);

    let encoder = engine
        .finish_recording()
        .expect("integral scroll must record as Native IR");
    assert!(encoder.commands().iter().any(|command| {
        matches!(
            command,
            crate::draw::pipeline::FrameCommand::Native {
                operation: FrameRasterOp::ScrollCopy {
                    dx: 0,
                    dy: 2,
                    ..
                }
            }
        )
    }));
}
