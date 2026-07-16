use crate::draw::pipeline::frame_recording::*;
use crate::draw::pipeline::{FrameCommand, FrameRadius, FrameRasterOp};
use crate::draw::primitives::types::{BlendMode, Radius, Transform};
use crate::draw::traits::GraphicsEngine;
use crate::tests::common::*;

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
fn transformed_recording_uses_a_tight_cpu_segment_at_the_mapped_bounds() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(20, 16).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine
        .canvas_2d()
        .set_transform(Transform::translate(2.0, 1.0).concat(Transform::scale(2.0, 2.0)));
    engine
        .canvas_2d()
        .fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::red(), None);

    let encoder = engine.finish_recording().expect("finish recorder");
    let segment = encoder
        .commands()
        .iter()
        .find_map(|command| match command {
            FrameCommand::CpuSegment { image, dst, .. } => Some((image, dst)),
            _ => None,
        })
        .expect("transformed CPU segment");
    assert_eq!((segment.0.width(), segment.0.height()), (4, 4));
    assert_eq!(*segment.1, FrameRect::new(4, 3, 4, 4));
    let reference = encoder.render_reference();
    assert_eq!(
        reference.pixel(3, 3),
        Some(Color::transparent().premultiplied())
    );
    assert_eq!(reference.pixel(4, 3), Some(Color::red().premultiplied()));
    assert_eq!(reference.pixel(7, 6), Some(Color::red().premultiplied()));
    assert_eq!(
        reference.pixel(8, 6),
        Some(Color::transparent().premultiplied())
    );
}

#[test]
fn clipped_unscaled_picture_stays_a_direct_picture_command() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(12, 8).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    let picture = engine.create_offscreen(6, 4).expect("Picture");
    engine
        .try_begin_offscreen_paint(&picture)
        .expect("begin Picture");
    engine
        .offscreen_canvas(&picture)
        .expect("Picture canvas")
        .fill_rect(Rect::new(0.0, 0.0, 6.0, 4.0), Color::green(), None);
    engine
        .try_end_offscreen_paint()
        .expect("end Picture target");
    engine.canvas_2d().push_clip(Rect::new(4.0, 2.0, 3.0, 2.0));
    engine
        .try_blit_offscreen_src(
            &picture,
            Rect::new(0.0, 0.0, 6.0, 4.0),
            Rect::new(2.0, 1.0, 6.0, 4.0),
        )
        .expect("record clipped Picture blit");
    engine.canvas_2d().pop_clip();

    let encoder = engine.finish_recording().expect("finish recorder");
    assert!(encoder.commands().iter().any(|command| {
        matches!(
            command,
            FrameCommand::PictureBlit { src, dst, .. }
                if *src == FrameRect::new(2, 1, 3, 2)
                    && *dst == FrameRect::new(4, 2, 3, 2)
        )
    }));
    assert!(!encoder
        .commands()
        .iter()
        .any(|command| matches!(command, FrameCommand::CpuSegment { .. })));
    let reference = encoder.render_reference();
    assert_eq!(
        reference.pixel(3, 2),
        Some(Color::transparent().premultiplied())
    );
    assert_eq!(reference.pixel(4, 2), Some(Color::green().premultiplied()));
    assert_eq!(reference.pixel(6, 3), Some(Color::green().premultiplied()));
    assert_eq!(
        reference.pixel(7, 3),
        Some(Color::transparent().premultiplied())
    );
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
fn additive_rounded_fill_records_validated_native_operation() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(8, 6).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine.canvas_2d().set_blend_mode(BlendMode::Additive);
    let radius = Radius {
        tl: 1.0,
        tr: 2.0,
        br: 3.0,
        bl: 4.0,
    };
    engine
        .canvas_2d()
        .fill_rect(Rect::new(1.0, 1.0, 6.0, 4.0), Color::red(), Some(radius));

    let encoder = engine
        .finish_recording()
        .expect("Additive rounded fill must record as validated Native IR");
    assert!(encoder.commands().iter().any(|command| {
        matches!(
            command,
            crate::draw::pipeline::FrameCommand::Native {
                operation: FrameRasterOp::FillRoundedRectAdditive {
                    rect,
                    color,
                    radius: recorded_radius,
                }
            } if *rect == FrameRect::new(1, 1, 6, 4)
                && *color == Color::red()
                && *recorded_radius == FrameRadius::new(radius).expect("valid radius")
        )
    }));
}

#[test]
fn additive_rounded_fill_rejects_invalid_radius_before_recording() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(8, 6).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine.canvas_2d().set_blend_mode(BlendMode::Additive);
    engine.canvas_2d().fill_rect(
        Rect::new(1.0, 1.0, 6.0, 4.0),
        Color::red(),
        Some(Radius::uniform(f32::NAN)),
    );

    let error = engine
        .finish_recording()
        .expect_err("invalid rounded Additive radius must remain typed");
    assert_eq!(error.code(), Errc::InvalidState);
}

#[test]
fn consecutive_cpu_draws_batch_into_one_segment_until_barrier() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(32, 16).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    // Two translucent CPU fills (rounded → not native) then an opaque native.
    engine.canvas_2d().fill_rect(
        Rect::new(2.0, 2.0, 4.0, 4.0),
        Color::from_rgba(255, 0, 0, 128),
        Some(Radius::uniform(1.0)),
    );
    engine.canvas_2d().fill_rect(
        Rect::new(8.0, 2.0, 4.0, 4.0),
        Color::from_rgba(0, 255, 0, 128),
        Some(Radius::uniform(1.0)),
    );
    engine
        .canvas_2d()
        .fill_rect(Rect::new(20.0, 2.0, 4.0, 4.0), Color::blue(), None);

    let encoder = engine.finish_recording().expect("finish recorder");
    let cpu_segments = encoder
        .commands()
        .iter()
        .filter(|command| {
            matches!(
                command,
                crate::draw::pipeline::FrameCommand::CpuSegment { .. }
            )
        })
        .count();
    assert_eq!(
        cpu_segments, 1,
        "consecutive CPU draws must share one CpuSegment until a native barrier"
    );
    assert!(encoder.commands().iter().any(|command| {
        matches!(
            command,
            crate::draw::pipeline::FrameCommand::Native {
                operation: FrameRasterOp::FillRect { .. }
            }
        )
    }));
}

#[test]
fn bounded_scratch_clear_prevents_pixels_leaking_across_barriers() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(40, 8).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    let first = Color::from_rgba(255, 0, 0, 128);
    engine.canvas_2d().fill_rect(
        Rect::new(8.0, 2.0, 4.0, 4.0),
        first,
        Some(Radius::uniform(1.0)),
    );
    engine
        .canvas_2d()
        .fill_rect(Rect::new(32.0, 2.0, 2.0, 2.0), Color::white(), None);
    engine.canvas_2d().fill_rect(
        Rect::new(1.0, 2.0, 3.0, 3.0),
        Color::from_rgba(0, 255, 0, 128),
        Some(Radius::uniform(1.0)),
    );
    engine.canvas_2d().fill_rect(
        Rect::new(20.0, 2.0, 3.0, 3.0),
        Color::from_rgba(0, 0, 255, 128),
        Some(Radius::uniform(1.0)),
    );

    let encoder = engine.finish_recording().expect("finish recorder");
    assert_eq!(
        encoder
            .commands()
            .iter()
            .filter(|command| matches!(command, FrameCommand::CpuSegment { .. }))
            .count(),
        2
    );
    assert_eq!(
        encoder.render_reference().pixel(9, 3),
        Some(first.premultiplied())
    );
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
                operation: FrameRasterOp::ScrollCopy { dx: 0, dy: 2, .. }
            }
        )
    }));
}
