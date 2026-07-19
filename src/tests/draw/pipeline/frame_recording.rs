use crate::draw::pipeline::frame_recording::*;
use crate::draw::pipeline::{
    EncodedPictureExecution, FrameCommand, FrameEncoder, FrameRadius, FrameRasterOp,
};
use crate::draw::primitives::types::{BlendMode, Radius, Transform};
use crate::draw::traits::{Canvas2D, GraphicsEngine};
use crate::tests::common::*;
use std::sync::Arc;

fn assert_premultiplied_pixels_within_one(expected: &[u32], actual: &[u32]) {
    assert_eq!(expected.len(), actual.len());
    for (index, (&expected, &actual)) in expected.iter().zip(actual).enumerate() {
        for shift in [24, 16, 8, 0] {
            let expected_channel = ((expected >> shift) & 0xff) as i16;
            let actual_channel = ((actual >> shift) & 0xff) as i16;
            assert!(
                (expected_channel - actual_channel).abs() <= 1,
                "pixel {index}, channel {shift}: expected {expected:#010X}, got {actual:#010X}"
            );
        }
    }
}

#[test]
fn native_only_recording_keeps_full_size_scratch_unallocated() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(1200, 800).expect("initialize recorder");
    assert_eq!(
        engine.canvas_2d().surface_size(),
        Size::new(1200.0, 800.0),
        "logical canvas size must not collapse to the placeholder extent"
    );
    assert_eq!(
        engine.scratch_surface_size(),
        (1, 1),
        "resize must retain only the lazy scratch placeholder"
    );
    assert_eq!(
        engine.memory_usage(),
        std::mem::size_of::<u32>(),
        "recorder resize must not allocate an unused CPU main surface"
    );
    let memory_before = engine.memory_usage();

    engine.begin_recording(true).expect("begin recording");
    engine
        .canvas_2d()
        .fill_rect(Rect::new(0.0, 0.0, 1200.0, 800.0), Color::white(), None);
    let encoder = engine.finish_recording().expect("finish recorder");

    assert!(encoder
        .commands()
        .iter()
        .any(|command| matches!(command, FrameCommand::Native { .. })));
    assert!(!encoder
        .commands()
        .iter()
        .any(|command| matches!(command, FrameCommand::CpuSegment { .. })));
    assert_eq!(engine.scratch_surface_size(), (1, 1));
    assert_eq!(engine.memory_usage(), memory_before);
}

#[test]
fn rounded_src_over_recording_stays_native_and_keeps_scratch_lazy() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(1200, 800).expect("initialize recorder");
    let memory_before = engine.memory_usage();
    let radius = Radius {
        tl: 12.0,
        tr: 8.0,
        br: 6.0,
        bl: 4.0,
    };

    engine.begin_recording(true).expect("begin recording");
    engine.canvas_2d().set_blend_mode(BlendMode::SrcOver);
    engine.canvas_2d().fill_rect(
        Rect::new(20.0, 30.0, 400.0, 240.0),
        Color::from_rgba(40, 120, 220, 160),
        Some(radius),
    );
    let encoder = engine.finish_recording().expect("finish recorder");

    assert!(encoder.commands().iter().any(|command| {
        matches!(
            command,
            FrameCommand::Native {
                operation: FrameRasterOp::FillRoundedRect {
                    rect,
                    color,
                    radius: recorded_radius,
                }
            } if *rect == FrameRect::new(20, 30, 400, 240)
                && *color == Color::from_rgba(40, 120, 220, 160)
                && *recorded_radius == FrameRadius::new(radius).expect("valid radius")
        )
    }));
    assert!(!encoder
        .commands()
        .iter()
        .any(|command| matches!(command, FrameCommand::CpuSegment { .. })));
    assert_eq!(engine.scratch_surface_size(), (1, 1));
    assert_eq!(engine.memory_usage(), memory_before);
}

#[test]
fn integral_surface_offsets_keep_sharp_and_rounded_fills_native_with_cpu_parity() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;

    let radius = Radius {
        tl: 3.0,
        tr: 2.0,
        br: 1.0,
        bl: 0.0,
    };
    let cases = [
        (
            (3.0, 2.0),
            Rect::new(1.0, 1.0, 4.0, 3.0),
            Rect::new(8.0, 2.0, 5.0, 4.0),
            FrameRect::new(4, 3, 4, 3),
            FrameRect::new(11, 4, 5, 4),
        ),
        (
            (-2.0, 1.0),
            Rect::new(3.0, 1.0, 4.0, 3.0),
            Rect::new(10.0, 2.0, 5.0, 4.0),
            FrameRect::new(1, 2, 4, 3),
            FrameRect::new(8, 3, 5, 4),
        ),
        (
            (0.5, 0.5),
            Rect::new(1.5, 1.5, 4.0, 3.0),
            Rect::new(8.5, 2.5, 5.0, 4.0),
            FrameRect::new(2, 2, 4, 3),
            FrameRect::new(9, 3, 5, 4),
        ),
    ];

    for (offset, sharp, rounded, expected_sharp, expected_rounded) in cases {
        let mut engine = FrameRecordingEngine::new();
        engine.initialize(20, 14).expect("initialize recorder");
        let memory_before = engine.memory_usage();
        engine.begin_recording(true).expect("begin recording");
        engine.canvas_2d().set_offset(offset.0, offset.1);
        engine
            .canvas_2d()
            .fill_rect(sharp, Color::from_rgba(220, 40, 20, 180), None);
        engine
            .canvas_2d()
            .fill_rect(rounded, Color::from_rgba(20, 80, 220, 160), Some(radius));
        let encoder = engine.finish_recording().expect("finish recorder");

        assert!(matches!(
            encoder.commands(),
            [
                FrameCommand::Clear { .. },
                FrameCommand::Native {
                    operation: FrameRasterOp::FillRect { rect: sharp, .. }
                },
                FrameCommand::Native {
                    operation: FrameRasterOp::FillRoundedRect { rect: rounded, .. }
                }
            ] if *sharp == expected_sharp && *rounded == expected_rounded
        ));
        assert_eq!(engine.scratch_surface_size(), (1, 1));
        assert_eq!(engine.memory_usage(), memory_before);

        let mut cpu = SharedRasterizer::new(PixelSurface::new(20, 14));
        cpu.set_offset(offset.0, offset.1);
        cpu.fill_rect(sharp, Color::from_rgba(220, 40, 20, 180), None);
        cpu.fill_rect(rounded, Color::from_rgba(20, 80, 220, 160), Some(radius));
        assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());
    }
}

#[test]
fn integral_offset_clip_is_frozen_in_surface_space_at_push_time() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;

    let local_clip = Rect::new(2.0, 2.0, 8.0, 6.0);
    let local_rect = Rect::new(0.0, 1.0, 10.0, 7.0);
    let offset = (4.0, 1.0);
    let radius = Radius::uniform(2.0);

    for (clip_before_offset, expected_clip) in [
        (true, FrameRect::new(2, 2, 8, 6)),
        (false, FrameRect::new(6, 3, 8, 6)),
    ] {
        let mut engine = FrameRecordingEngine::new();
        engine.initialize(20, 14).expect("initialize recorder");
        engine.begin_recording(true).expect("begin recording");
        if clip_before_offset {
            engine.canvas_2d().push_clip(local_clip);
        }
        engine.canvas_2d().set_offset(offset.0, offset.1);
        if !clip_before_offset {
            engine.canvas_2d().push_clip(local_clip);
        }
        engine.canvas_2d().fill_rect(
            local_rect,
            Color::from_rgba(40, 160, 220, 176),
            Some(radius),
        );
        let encoder = engine.finish_recording().expect("finish recorder");

        assert!(matches!(
            encoder.commands(),
            [
                FrameCommand::Clear { .. },
                FrameCommand::Native {
                    operation: FrameRasterOp::FillRoundedRectClipped { rect, clip, .. }
                }
            ] if *rect == FrameRect::new(4, 2, 10, 7) && *clip == expected_clip
        ));
        assert_eq!(engine.scratch_surface_size(), (1, 1));

        let mut cpu = SharedRasterizer::new(PixelSurface::new(20, 14));
        if clip_before_offset {
            cpu.push_clip(local_clip);
        }
        cpu.set_offset(offset.0, offset.1);
        if !clip_before_offset {
            cpu.push_clip(local_clip);
        }
        cpu.fill_rect(
            local_rect,
            Color::from_rgba(40, 160, 220, 176),
            Some(radius),
        );
        assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());
    }
}

#[test]
fn nested_integral_clip_keeps_rounded_and_sharp_src_over_fills_native() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(1200, 800).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine
        .canvas_2d()
        .push_clip(Rect::new(10.0, 20.0, 100.0, 60.0));
    engine
        .canvas_2d()
        .push_clip(Rect::new(20.0, 10.0, 50.0, 50.0));
    engine.canvas_2d().fill_rect(
        Rect::new(8.0, 8.0, 100.0, 80.0),
        Color::from_rgba(40, 120, 220, 160),
        Some(Radius::uniform(12.0)),
    );
    engine
        .canvas_2d()
        .fill_rect(Rect::new(12.0, 22.0, 20.0, 10.0), Color::white(), None);
    let encoder = engine.finish_recording().expect("finish recorder");

    let clipped = encoder
        .commands()
        .iter()
        .filter_map(|command| match command {
            FrameCommand::Native {
                operation:
                    FrameRasterOp::FillRoundedRectClipped {
                        rect, radius, clip, ..
                    },
            } => Some((*rect, radius.to_radius(), *clip)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(clipped.len(), 2);
    assert_eq!(clipped[0].0, FrameRect::new(8, 8, 100, 80));
    assert_eq!(clipped[0].2, FrameRect::new(20, 20, 50, 40));
    assert_eq!(clipped[1].1, Radius::uniform(0.0));
    assert!(!encoder
        .commands()
        .iter()
        .any(|command| matches!(command, FrameCommand::CpuSegment { .. })));
    assert_eq!(engine.scratch_surface_size(), (1, 1));
}

#[test]
fn fractional_clip_keeps_rounded_src_over_fill_on_cpu() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(64, 48).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine.canvas_2d().push_clip(Rect::new(12.5, 1.0, 4.0, 8.0));
    engine.canvas_2d().fill_rect(
        Rect::new(12.0, 1.0, 8.0, 8.0),
        Color::green(),
        Some(Radius::uniform(2.0)),
    );
    let encoder = engine.finish_recording().expect("finish recorder");

    assert_eq!(engine.scratch_surface_size(), (64, 48));
    assert_eq!(
        encoder
            .commands()
            .iter()
            .filter(|command| matches!(command, FrameCommand::CpuSegment { .. }))
            .count(),
        1
    );
    assert!(!encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::Native {
            operation: FrameRasterOp::FillRoundedRectClipped { .. }
        }
    )));
}

#[test]
fn clipped_native_rounded_fill_is_a_painter_order_barrier_for_cpu_batches() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;

    let mut engine = FrameRecordingEngine::new();
    engine.initialize(64, 32).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine
        .canvas_2d()
        .fill_ellipse(Rect::new(1.0, 1.0, 6.0, 6.0), Color::red());
    engine
        .canvas_2d()
        .push_clip(Rect::new(12.0, 2.0, 10.0, 8.0));
    engine.canvas_2d().set_offset(2.0, 1.0);
    engine.canvas_2d().fill_rect(
        Rect::new(10.0, 0.0, 16.0, 12.0),
        Color::green(),
        Some(Radius::uniform(3.0)),
    );
    engine.canvas_2d().set_offset(0.0, 0.0);
    engine.canvas_2d().pop_clip();
    engine
        .canvas_2d()
        .fill_ellipse(Rect::new(30.0, 1.0, 6.0, 6.0), Color::blue());
    let encoder = engine.finish_recording().expect("finish recorder");

    assert!(matches!(
        encoder.commands(),
        [
            FrameCommand::Clear { .. },
            FrameCommand::CpuSegment { .. },
            FrameCommand::Native {
                operation: FrameRasterOp::FillRoundedRectClipped { rect, clip, .. }
            },
            FrameCommand::CpuSegment { .. }
        ] if *rect == FrameRect::new(12, 1, 16, 12)
            && *clip == FrameRect::new(12, 2, 10, 8)
    ));

    let mut cpu = SharedRasterizer::new(PixelSurface::new(64, 32));
    cpu.fill_ellipse(Rect::new(1.0, 1.0, 6.0, 6.0), Color::red());
    cpu.push_clip(Rect::new(12.0, 2.0, 10.0, 8.0));
    cpu.set_offset(2.0, 1.0);
    cpu.fill_rect(
        Rect::new(10.0, 0.0, 16.0, 12.0),
        Color::green(),
        Some(Radius::uniform(3.0)),
    );
    cpu.set_offset(0.0, 0.0);
    cpu.pop_clip();
    cpu.fill_ellipse(Rect::new(30.0, 1.0, 6.0, 6.0), Color::blue());
    assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());
}

#[test]
fn eligible_shared_glyphs_batch_as_native_ir_and_keep_scratch_lazy() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(1200, 800).expect("initialize recorder");
    let coverage: Arc<[u8]> = vec![0, 64, 192, 255, 255, 192, 64, 0].into();
    engine.begin_recording(true).expect("begin recording");
    engine
        .canvas_2d()
        .push_clip(Rect::new(10.0, 20.0, 100.0, 40.0));
    engine.canvas_2d().blit_glyph_shared(
        12,
        22,
        Arc::clone(&coverage),
        4,
        2,
        Color::from_rgba(20, 40, 60, 180),
    );
    engine
        .canvas_2d()
        .blit_glyph_shared(18, 22, Arc::clone(&coverage), 4, 2, Color::white());
    let encoder = engine.finish_recording().expect("finish recorder");

    assert_eq!(engine.scratch_surface_size(), (1, 1));
    assert!(matches!(
        encoder.commands(),
        [
            FrameCommand::Clear { .. },
            FrameCommand::Native {
                operation: FrameRasterOp::BlitGlyphs { glyphs, clip }
            }
        ] if glyphs.len() == 2
            && *clip == FrameRect::new(10, 20, 100, 40)
            && Arc::ptr_eq(glyphs[0].coverage(), &coverage)
            && Arc::ptr_eq(glyphs[1].coverage(), &coverage)
    ));
}

#[test]
fn glyph_opacity_stays_native_and_matches_cpu_before_coverage_quantization() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;

    let coverage: Arc<[u8]> = vec![1, 64, 127, 128, 254, 255, 32, 192].into();
    let glyph_color = Color::from_rgba(17, 83, 201, 173);
    let background = Color::from_rgb(24, 48, 72);
    let clip = Rect::new(2.0, 2.0, 12.0, 6.0);

    for opacity in [f32::NAN, 0.0, 1.0 / 255.0, 0.37, 0.5, 0.999_998, 1.0] {
        let mut engine = FrameRecordingEngine::new();
        engine.initialize(20, 12).expect("initialize recorder");
        engine.begin_recording(true).expect("begin recording");
        engine
            .canvas_2d()
            .fill_rect(Rect::new(0.0, 0.0, 20.0, 12.0), background, None);
        engine.canvas_2d().set_opacity(opacity);
        engine.canvas_2d().push_clip(clip);
        engine
            .canvas_2d()
            .blit_glyph_shared(3, 3, Arc::clone(&coverage), 4, 2, glyph_color);
        let encoder = engine.finish_recording().expect("finish recorder");
        let expected_color =
            crate::draw::rasterizer::color_with_glyph_opacity(glyph_color, opacity);
        let recorded = encoder.commands().iter().find_map(|command| match command {
            FrameCommand::Native {
                operation: FrameRasterOp::BlitGlyphs { glyphs, clip },
            } => Some((glyphs, clip)),
            _ => None,
        });
        if expected_color.a == 0 {
            assert!(
                recorded.is_none(),
                "zero-alpha glyph opacity must be a no-op"
            );
        } else {
            let (glyphs, recorded_clip) = recorded.expect("opacity glyph must remain native");
            assert_eq!(glyphs.len(), 1);
            assert_eq!(glyphs[0].color(), expected_color);
            assert_eq!(*recorded_clip, FrameRect::new(2, 2, 12, 6));
            assert!(Arc::ptr_eq(glyphs[0].coverage(), &coverage));
        }
        assert_eq!(engine.scratch_surface_size(), (1, 1));
        assert!(!encoder
            .commands()
            .iter()
            .any(|command| matches!(command, FrameCommand::CpuSegment { .. })));

        let mut cpu = SharedRasterizer::new(PixelSurface::new(20, 12));
        cpu.fill_rect(Rect::new(0.0, 0.0, 20.0, 12.0), background, None);
        cpu.set_opacity(opacity);
        cpu.push_clip(clip);
        cpu.blit_glyph(3, 3, coverage.as_ref(), 4, 2, glyph_color);
        assert_eq!(
            encoder.render_reference().pixels(),
            cpu.surface().pixels(),
            "opacity={opacity:?}"
        );
    }
}

#[test]
fn integral_picture_origin_splices_opacity_adjusted_shared_glyph_ir_into_the_main_frame() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;

    let mut engine = FrameRecordingEngine::new();
    engine.initialize(96, 48).expect("initialize recorder");
    let picture = engine.create_offscreen(64, 32).expect("Picture");
    let coverage: Arc<[u8]> = vec![0, 64, 128, 255, 255, 128, 64, 0].into();
    let glyph_color = Color::from_rgba(220, 96, 40, 160);

    engine
        .try_begin_offscreen_paint(&picture)
        .expect("begin Picture");
    {
        let canvas = engine.offscreen_canvas(&picture).expect("Picture canvas");
        canvas.translate(-10.0, -5.0);
        canvas.set_opacity(0.37);
        canvas.blit_glyph_shared(12, 7, Arc::clone(&coverage), 4, 2, glyph_color);
    }
    engine
        .try_end_offscreen_paint()
        .expect("commit Picture recorder");
    assert_eq!(
        engine.offscreen_scratch_surface_size(&picture),
        Some((1, 1))
    );

    engine.begin_recording(true).expect("begin main frame");
    engine.canvas_2d().fill_rect(
        Rect::new(0.0, 0.0, 96.0, 48.0),
        Color::from_rgb(12, 24, 48),
        None,
    );
    engine
        .try_blit_offscreen_src(
            &picture,
            Rect::new(0.0, 0.0, 64.0, 32.0),
            Rect::new(10.0, 5.0, 64.0, 32.0),
        )
        .expect("splice Picture glyph IR");
    let encoder = engine.finish_recording().expect("finish main frame");

    let glyphs = encoder
        .commands()
        .iter()
        .find_map(|command| match command {
            FrameCommand::Native {
                operation: FrameRasterOp::BlitGlyphs { glyphs, clip },
            } if *clip == FrameRect::new(10, 5, 64, 32) => Some(glyphs),
            _ => None,
        })
        .expect("Picture glyphs must reach the main encoder");
    assert_eq!((glyphs[0].x(), glyphs[0].y()), (12, 7));
    assert_eq!(
        glyphs[0].color(),
        crate::draw::rasterizer::color_with_glyph_opacity(glyph_color, 0.37)
    );
    assert!(Arc::ptr_eq(glyphs[0].coverage(), &coverage));
    assert!(!encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::PictureBlit { .. } | FrameCommand::CpuSegment { .. }
    )));
    assert_eq!(engine.scratch_surface_size(), (1, 1));

    let mut cpu = SharedRasterizer::new(PixelSurface::try_new(96, 48).expect("CPU surface"));
    cpu.fill_rect(
        Rect::new(0.0, 0.0, 96.0, 48.0),
        Color::from_rgb(12, 24, 48),
        None,
    );
    cpu.set_opacity(0.37);
    cpu.blit_glyph(12, 7, coverage.as_ref(), 4, 2, glyph_color);
    assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());
}

#[test]
fn opaque_rounded_picture_background_splices_overlapping_glyphs_without_materializing() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;

    let mut engine = FrameRecordingEngine::new();
    engine.initialize(48, 28).expect("initialize recorder");
    let picture = engine.create_offscreen(32, 16).expect("Picture");
    let coverage: Arc<[u8]> = vec![0, 64, 192, 255, 255, 192, 64, 0].into();
    let picture_background = Color::from_rgb(24, 48, 72);
    let first_color = Color::from_rgba(240, 80, 40, 192);
    let second_color = Color::from_rgba(40, 200, 240, 160);

    engine
        .try_begin_offscreen_paint(&picture)
        .expect("begin Picture");
    {
        let canvas = engine.offscreen_canvas(&picture).expect("Picture canvas");
        canvas.fill_rect(
            Rect::new(0.0, 0.0, 32.0, 16.0),
            picture_background,
            Some(Radius::uniform(4.0)),
        );
        canvas.blit_glyph_shared(6, 6, Arc::clone(&coverage), 4, 2, first_color);
        canvas.blit_glyph_shared(8, 6, Arc::clone(&coverage), 4, 2, second_color);
    }
    engine
        .try_end_offscreen_paint()
        .expect("commit opaque Picture");

    let parent_background = Color::from_rgb(96, 24, 12);
    engine.begin_recording(true).expect("begin main frame");
    engine
        .canvas_2d()
        .fill_rect(Rect::new(0.0, 0.0, 48.0, 28.0), parent_background, None);
    engine
        .try_blit_offscreen_src(
            &picture,
            Rect::new(0.0, 0.0, 32.0, 16.0),
            Rect::new(5.0, 4.0, 32.0, 16.0),
        )
        .expect("splice opaque-backed Picture glyph IR");
    let encoder = engine.finish_recording().expect("finish main frame");

    let glyphs = encoder
        .commands()
        .iter()
        .find_map(|command| match command {
            FrameCommand::Native {
                operation: FrameRasterOp::BlitGlyphs { glyphs, .. },
            } => Some(glyphs),
            _ => None,
        })
        .expect("overlapping Picture glyphs must remain native");
    assert_eq!(glyphs.len(), 2);
    assert!(Arc::ptr_eq(glyphs[0].coverage(), &coverage));
    assert!(Arc::ptr_eq(glyphs[1].coverage(), &coverage));
    assert!(!encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::PictureBlit { .. } | FrameCommand::CpuSegment { .. }
    )));
    assert_eq!(engine.scratch_surface_size(), (1, 1));
    assert_eq!(
        engine.offscreen_scratch_surface_size(&picture),
        Some((1, 1))
    );

    let mut source = SharedRasterizer::new(PixelSurface::try_new(32, 16).unwrap());
    source.fill_rect(
        Rect::new(0.0, 0.0, 32.0, 16.0),
        picture_background,
        Some(Radius::uniform(4.0)),
    );
    source.blit_glyph(6, 6, coverage.as_ref(), 4, 2, first_color);
    source.blit_glyph(8, 6, coverage.as_ref(), 4, 2, second_color);
    let mut expected = SharedRasterizer::new(PixelSurface::try_new(48, 28).unwrap());
    expected.fill_rect(Rect::new(0.0, 0.0, 48.0, 28.0), parent_background, None);
    expected.blit_image(
        source.surface().pixels(),
        32,
        Rect::new(0.0, 0.0, 32.0, 16.0),
        Rect::new(5.0, 4.0, 32.0, 16.0),
    );
    assert_premultiplied_pixels_within_one(
        expected.surface().pixels(),
        encoder.render_reference().pixels(),
    );
}

#[test]
fn adjacent_opaque_picture_tiles_splice_glyph_overlap_across_their_seam() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;

    let mut engine = FrameRecordingEngine::new();
    engine.initialize(48, 28).expect("initialize recorder");
    let picture = engine.create_offscreen(32, 16).expect("Picture");
    let coverage: Arc<[u8]> = vec![0, 64, 192, 255, 255, 192, 64, 0].into();
    let left_background = Color::from_rgb(24, 48, 72);
    let right_background = Color::from_rgb(72, 48, 24);
    let first_color = Color::from_rgba(240, 80, 40, 192);
    let second_color = Color::from_rgba(40, 200, 240, 160);

    engine
        .try_begin_offscreen_paint(&picture)
        .expect("begin Picture");
    {
        let canvas = engine.offscreen_canvas(&picture).expect("Picture canvas");
        canvas.fill_rect(Rect::new(0.0, 0.0, 16.0, 16.0), left_background, None);
        canvas.fill_rect(Rect::new(16.0, 0.0, 16.0, 16.0), right_background, None);
        canvas.blit_glyph_shared(13, 6, Arc::clone(&coverage), 4, 2, first_color);
        canvas.blit_glyph_shared(15, 6, Arc::clone(&coverage), 4, 2, second_color);
    }
    engine
        .try_end_offscreen_paint()
        .expect("commit tiled Picture");

    let parent_background = Color::from_rgb(96, 24, 12);
    engine.begin_recording(true).expect("begin main frame");
    engine
        .canvas_2d()
        .fill_rect(Rect::new(0.0, 0.0, 48.0, 28.0), parent_background, None);
    engine
        .try_blit_offscreen_src(
            &picture,
            Rect::new(0.0, 0.0, 32.0, 16.0),
            Rect::new(5.0, 4.0, 32.0, 16.0),
        )
        .expect("splice seam-backed Picture glyph IR");
    let encoder = engine.finish_recording().expect("finish main frame");

    let glyphs = encoder
        .commands()
        .iter()
        .find_map(|command| match command {
            FrameCommand::Native {
                operation: FrameRasterOp::BlitGlyphs { glyphs, .. },
            } => Some(glyphs),
            _ => None,
        })
        .expect("glyphs crossing the opaque tile seam must remain native");
    assert_eq!(glyphs.len(), 2);
    assert!(Arc::ptr_eq(glyphs[0].coverage(), &coverage));
    assert!(Arc::ptr_eq(glyphs[1].coverage(), &coverage));
    assert!(!encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::PictureBlit { .. } | FrameCommand::CpuSegment { .. }
    )));
    assert_eq!(engine.scratch_surface_size(), (1, 1));
    assert_eq!(
        engine.offscreen_scratch_surface_size(&picture),
        Some((1, 1))
    );

    let mut source = SharedRasterizer::new(PixelSurface::try_new(32, 16).unwrap());
    source.fill_rect(Rect::new(0.0, 0.0, 16.0, 16.0), left_background, None);
    source.fill_rect(Rect::new(16.0, 0.0, 16.0, 16.0), right_background, None);
    source.blit_glyph(13, 6, coverage.as_ref(), 4, 2, first_color);
    source.blit_glyph(15, 6, coverage.as_ref(), 4, 2, second_color);
    let mut expected = SharedRasterizer::new(PixelSurface::try_new(48, 28).unwrap());
    expected.fill_rect(Rect::new(0.0, 0.0, 48.0, 28.0), parent_background, None);
    expected.blit_image(
        source.surface().pixels(),
        32,
        Rect::new(0.0, 0.0, 32.0, 16.0),
        Rect::new(5.0, 4.0, 32.0, 16.0),
    );
    assert_eq!(
        expected.surface().pixels(),
        encoder.render_reference().pixels(),
        "opaque tile-union grouping must stay bit-exact"
    );
}

#[test]
fn integral_picture_crop_splices_rounded_and_glyph_ir_without_materializing() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;

    let mut engine = FrameRecordingEngine::new();
    engine.initialize(40, 24).expect("initialize recorder");
    let picture = engine.create_offscreen(32, 16).expect("Picture");
    let coverage: Arc<[u8]> = vec![0, 64, 128, 255, 255, 128, 64, 0].into();
    let rounded_color = Color::from_rgba(220, 96, 40, 160);
    let glyph_color = Color::from_rgba(40, 180, 240, 192);
    let radius = Radius::uniform(3.0);

    engine
        .try_begin_offscreen_paint(&picture)
        .expect("begin Picture");
    {
        let canvas = engine.offscreen_canvas(&picture).expect("Picture canvas");
        canvas.fill_rect(Rect::new(2.0, 2.0, 8.0, 8.0), rounded_color, Some(radius));
        canvas.blit_glyph_shared(14, 2, Arc::clone(&coverage), 4, 2, glyph_color);
    }
    engine
        .try_end_offscreen_paint()
        .expect("commit Picture recorder");

    engine.begin_recording(true).expect("begin main frame");
    let background = Color::from_rgb(12, 24, 48);
    engine
        .canvas_2d()
        .fill_rect(Rect::new(0.0, 0.0, 40.0, 24.0), background, None);
    engine
        .try_blit_offscreen_src(
            &picture,
            Rect::new(6.0, 0.0, 16.0, 12.0),
            Rect::new(4.0, 3.0, 16.0, 12.0),
        )
        .expect("splice cropped Picture IR");
    let encoder = engine.finish_recording().expect("finish main frame");

    assert!(encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::Native {
            operation: FrameRasterOp::FillRoundedRectClipped { .. }
        }
    )));
    let glyphs = encoder
        .commands()
        .iter()
        .find_map(|command| match command {
            FrameCommand::Native {
                operation: FrameRasterOp::BlitGlyphs { glyphs, .. },
            } => Some(glyphs),
            _ => None,
        })
        .expect("cropped glyph batch stays native");
    assert!(Arc::ptr_eq(glyphs[0].coverage(), &coverage));
    assert!(!encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::PictureBlit { .. } | FrameCommand::CpuSegment { .. }
    )));
    assert_eq!(engine.scratch_surface_size(), (1, 1));
    assert_eq!(
        engine.offscreen_scratch_surface_size(&picture),
        Some((1, 1))
    );

    let mut source = SharedRasterizer::new(PixelSurface::try_new(32, 16).unwrap());
    source.fill_rect(Rect::new(2.0, 2.0, 8.0, 8.0), rounded_color, Some(radius));
    source.blit_glyph(14, 2, coverage.as_ref(), 4, 2, glyph_color);
    let mut expected = SharedRasterizer::new(PixelSurface::try_new(40, 24).unwrap());
    expected.fill_rect(Rect::new(0.0, 0.0, 40.0, 24.0), background, None);
    expected.blit_image(
        source.surface().pixels(),
        32,
        Rect::new(6.0, 0.0, 16.0, 12.0),
        Rect::new(4.0, 3.0, 16.0, 12.0),
    );
    assert_premultiplied_pixels_within_one(
        expected.surface().pixels(),
        encoder.render_reference().pixels(),
    );
}

#[test]
fn fractional_picture_origin_and_scaled_blit_keep_strict_materialized_fallbacks() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(96, 48).expect("initialize recorder");
    let picture = engine.create_offscreen(64, 32).expect("Picture");
    let coverage: Arc<[u8]> = vec![255; 8].into();
    engine
        .try_begin_offscreen_paint(&picture)
        .expect("begin Picture");
    {
        let canvas = engine.offscreen_canvas(&picture).expect("Picture canvas");
        canvas.translate(-10.5, -5.0);
        canvas.blit_glyph_shared(12, 7, coverage, 4, 2, Color::white());
    }
    engine
        .try_end_offscreen_paint()
        .expect("commit fractional Picture");

    engine.begin_recording(true).expect("begin main frame");
    engine
        .try_blit_offscreen_src(
            &picture,
            Rect::new(0.0, 0.0, 64.0, 32.0),
            Rect::new(4.0, 3.0, 32.0, 16.0),
        )
        .expect("record scaled Picture fallback");
    let encoder = engine.finish_recording().expect("finish main frame");
    assert!(encoder
        .commands()
        .iter()
        .any(|command| matches!(command, FrameCommand::PictureBlit { .. })));
    assert!(!encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::Native {
            operation: FrameRasterOp::BlitGlyphs { .. }
        }
    )));
}

#[test]
fn picture_retained_ir_obeys_bgra_budget_and_destroy_releases_coverage() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(96, 48).expect("initialize recorder");

    let retained = engine.create_offscreen(64, 32).expect("retained Picture");
    let coverage: Arc<[u8]> = vec![255; 8].into();
    let weak = Arc::downgrade(&coverage);
    engine
        .try_begin_offscreen_paint(&retained)
        .expect("begin retained Picture");
    engine
        .offscreen_canvas(&retained)
        .expect("retained Picture canvas")
        .blit_glyph_shared(2, 2, Arc::clone(&coverage), 4, 2, Color::white());
    engine
        .try_end_offscreen_paint()
        .expect("commit retained Picture");
    drop(coverage);
    assert!(weak.upgrade().is_some(), "committed IR owns coverage");
    engine.destroy_offscreen(retained);
    assert!(
        weak.upgrade().is_none(),
        "destroy releases retained coverage"
    );

    let bounded = engine.create_offscreen(64, 32).expect("bounded Picture");
    engine
        .try_begin_offscreen_paint(&bounded)
        .expect("begin oversized IR Picture");
    for index in 0..64 {
        let large_coverage: Arc<[u8]> = vec![index as u8; 16 * 16].into();
        engine
            .offscreen_canvas(&bounded)
            .expect("bounded Picture canvas")
            .blit_glyph_shared(4, 4, large_coverage, 16, 16, Color::white());
    }
    engine
        .try_end_offscreen_paint()
        .expect("commit bounded Picture");
    assert_eq!(
        engine.memory_usage(),
        2 * std::mem::size_of::<u32>() + 64 * 32 * std::mem::size_of::<u32>(),
        "oversized retained IR becomes one BGRA image plus 1x1 main/Picture scratch"
    );

    engine.begin_recording(true).expect("begin main frame");
    engine
        .try_blit_offscreen_src(
            &bounded,
            Rect::new(0.0, 0.0, 64.0, 32.0),
            Rect::new(0.0, 0.0, 64.0, 32.0),
        )
        .expect("record bounded Picture");
    let encoder = engine.finish_recording().expect("finish bounded frame");
    assert!(encoder
        .commands()
        .iter()
        .any(|command| matches!(command, FrameCommand::PictureBlit { .. })));
}

#[test]
fn fractional_glyph_clip_remains_cpu_and_malformed_glyph_is_noop() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(32, 16).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine
        .canvas_2d()
        .blit_glyph(0, 0, &[255; 3], 2, 2, Color::red());
    assert_eq!(engine.scratch_surface_size(), (1, 1));
    engine.canvas_2d().push_clip(Rect::new(1.5, 1.0, 8.0, 4.0));
    engine.canvas_2d().blit_glyph(
        1,
        1,
        &[255, 128, 64, 0, 0, 64, 128, 255],
        4,
        2,
        Color::white(),
    );
    let encoder = engine.finish_recording().expect("finish recorder");
    assert_eq!(engine.scratch_surface_size(), (32, 16));
    assert!(encoder
        .commands()
        .iter()
        .any(|command| matches!(command, FrameCommand::CpuSegment { .. })));
    assert!(!encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::Native {
            operation: FrameRasterOp::BlitGlyphs { .. }
        }
    )));
}

#[test]
fn fill_opacity_stays_native_and_preserves_exact_cpu_premultiplied_sources() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;

    let sharp_color = Color::from_rgba(17, 83, 201, 173);
    let rounded_color = Color::from_rgba(211, 47, 129, 149);
    let sharp_rect = Rect::new(2.0, 2.0, 8.0, 6.0);
    let rounded_rect = Rect::new(14.0, 1.0, 12.0, 9.0);
    let clip = Rect::new(15.0, 2.0, 9.0, 7.0);
    let radius = Radius::uniform(3.0);

    for opacity in [0.0, 0.37, 0.5, 0.999_998] {
        let mut engine = FrameRecordingEngine::new();
        engine.initialize(32, 16).expect("initialize recorder");
        let memory_before = engine.memory_usage();
        engine.begin_recording(true).expect("begin recording");
        engine.canvas_2d().set_opacity(opacity);
        engine.canvas_2d().fill_rect(sharp_rect, sharp_color, None);
        engine.canvas_2d().push_clip(clip);
        engine
            .canvas_2d()
            .fill_rect(rounded_rect, rounded_color, Some(radius));
        let encoder = engine.finish_recording().expect("finish recorder");

        let expected_sharp =
            crate::draw::rasterizer::apply_opacity(sharp_color.premultiplied(), opacity);
        let expected_rounded =
            crate::draw::rasterizer::apply_opacity(rounded_color.premultiplied(), opacity);
        assert!(matches!(
            encoder.commands(),
            [
                FrameCommand::Clear { .. },
                FrameCommand::Native {
                    operation: FrameRasterOp::FillRect { color: sharp, .. }
                },
                FrameCommand::Native {
                    operation: FrameRasterOp::FillRoundedRectClipped {
                        color: rounded,
                        clip: recorded_clip,
                        ..
                    }
                }
            ] if sharp.premultiplied() == expected_sharp
                && rounded.premultiplied() == expected_rounded
                && *recorded_clip == FrameRect::new(15, 2, 9, 7)
        ));
        assert_eq!(engine.scratch_surface_size(), (1, 1));
        assert_eq!(engine.memory_usage(), memory_before);

        let mut cpu = SharedRasterizer::new(PixelSurface::new(32, 16));
        cpu.set_opacity(opacity);
        cpu.fill_rect(sharp_rect, sharp_color, None);
        cpu.push_clip(clip);
        cpu.fill_rect(rounded_rect, rounded_color, Some(radius));
        assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());
    }
}

#[test]
fn rounded_src_over_non_exact_geometries_remain_cpu_fallbacks() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(64, 48).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    let radius = Some(Radius::uniform(2.0));

    engine
        .canvas_2d()
        .fill_rect(Rect::new(1.25, 1.0, 8.0, 8.0), Color::red(), radius);
    engine
        .canvas_2d()
        .push_clip(Rect::new(12.25, 1.0, 4.0, 8.0));
    engine
        .canvas_2d()
        .fill_rect(Rect::new(12.0, 1.0, 8.0, 8.0), Color::green(), radius);
    engine.canvas_2d().pop_clip();
    engine
        .canvas_2d()
        .set_transform(Transform::translate(1.0, 0.0));
    engine
        .canvas_2d()
        .fill_rect(Rect::new(22.0, 1.0, 8.0, 8.0), Color::blue(), radius);
    engine.canvas_2d().set_transform(Transform::identity());
    engine.canvas_2d().set_offset(0.5, 0.0);
    engine
        .canvas_2d()
        .fill_rect(Rect::new(32.0, 1.0, 8.0, 8.0), Color::white(), radius);
    let encoder = engine.finish_recording().expect("finish recorder");
    assert!(encoder
        .commands()
        .iter()
        .any(|command| matches!(command, FrameCommand::CpuSegment { .. })));
    assert!(!encoder.commands().iter().any(|command| {
        matches!(
            command,
            FrameCommand::Native {
                operation: FrameRasterOp::FillRoundedRect { .. }
            }
        )
    }));
}

#[test]
fn ordinary_invalid_radius_with_integral_offset_remains_an_exact_cpu_fallback() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;

    let mut engine = FrameRecordingEngine::new();
    engine.initialize(16, 12).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine.canvas_2d().set_offset(2.0, 1.0);
    let rect = Rect::new(2.0, 2.0, 8.0, 6.0);
    let color = Color::from_rgba(30, 120, 220, 176);
    let invalid_radius = Some(Radius::uniform(-1.0));
    engine.canvas_2d().fill_rect(rect, color, invalid_radius);
    let encoder = engine
        .finish_recording()
        .expect("ordinary invalid radius preserves the prior void CPU behavior");

    assert_eq!(engine.scratch_surface_size(), (16, 12));
    assert!(encoder
        .commands()
        .iter()
        .any(|command| matches!(command, FrameCommand::CpuSegment { .. })));
    assert!(!encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::Native {
            operation: FrameRasterOp::FillRect { .. }
                | FrameRasterOp::FillRoundedRect { .. }
                | FrameRasterOp::FillRoundedRectClipped { .. }
        }
    )));

    let mut cpu = SharedRasterizer::new(PixelSurface::new(16, 12));
    cpu.set_offset(2.0, 1.0);
    cpu.fill_rect(rect, color, invalid_radius);
    assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());
}

#[test]
fn first_cpu_draw_allocates_scratch_and_resize_or_shutdown_releases_it() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(20, 16).expect("initialize recorder");
    assert_eq!(engine.scratch_surface_size(), (1, 1));
    let memory_before = engine.memory_usage();
    engine.begin_recording(true).expect("begin recording");
    engine
        .canvas_2d()
        .set_transform(Transform::translate(2.0, 1.0));
    engine.canvas_2d().push_clip(Rect::new(1.0, 1.0, 4.0, 4.0));
    engine.canvas_2d().fill_rect(
        Rect::new(1.0, 1.0, 8.0, 6.0),
        Color::from_rgba(20, 40, 60, 128),
        Some(Radius::uniform(1.0)),
    );
    let encoder = engine.finish_recording().expect("finish recorder");

    assert_eq!(engine.scratch_surface_size(), (20, 16));
    assert_eq!(
        engine.memory_usage() - memory_before,
        (20 * 16 - 1) * std::mem::size_of::<u32>(),
        "memory diagnostics must expose the lazily allocated scratch bytes"
    );
    assert!(encoder
        .commands()
        .iter()
        .any(|command| matches!(command, FrameCommand::CpuSegment { .. })));
    assert_eq!(
        encoder.render_reference().pixel(4, 3),
        Some(Color::from_rgba(20, 40, 60, 128).premultiplied()),
        "lazy allocation must preserve the transform and clip configured before the draw"
    );

    engine.resize(12, 10).expect("resize recorder");
    assert_eq!(
        engine.scratch_surface_size(),
        (1, 1),
        "resize must release an existing full-size scratch allocation"
    );

    engine.begin_recording(true).expect("begin after resize");
    engine.canvas_2d().fill_circle(4.0, 4.0, 2.0, Color::blue());
    engine.finish_recording().expect("finish after resize");
    assert_eq!(engine.scratch_surface_size(), (12, 10));
    engine.try_shutdown().expect("shutdown recorder");
    assert_eq!(
        engine.scratch_surface_size(),
        (1, 1),
        "shutdown must release an existing full-size scratch allocation"
    );
}

#[test]
fn recording_canvas_emits_native_cpu_and_picture_commands_in_painter_order() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(12, 8).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine
        .canvas_2d()
        .fill_rect(Rect::new(1.0, 1.0, 2.0, 2.0), Color::red(), None);
    engine.canvas_2d().fill_ellipse(
        Rect::new(4.0, 1.0, 2.0, 2.0),
        Color::from_rgba(0, 120, 255, 128),
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
fn integral_offset_picture_splices_native_ir_without_allocating_scratch() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(24, 16).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    let picture = engine.create_offscreen(16, 8).expect("Picture");
    let coverage: Arc<[u8]> = vec![0, 64, 128, 255, 255, 128, 64, 0].into();
    engine
        .try_begin_offscreen_paint(&picture)
        .expect("begin Picture");
    {
        let canvas = engine.offscreen_canvas(&picture).expect("Picture canvas");
        canvas.fill_rect(Rect::new(0.0, 0.0, 2.0, 2.0), Color::green(), None);
        canvas.blit_glyph_shared(5, 1, Arc::clone(&coverage), 4, 2, Color::white());
    }
    engine
        .try_end_offscreen_paint()
        .expect("end Picture target");
    engine.canvas_2d().set_offset(3.0, 2.0);

    engine
        .try_blit_offscreen_src(
            &picture,
            Rect::new(0.0, 0.0, 16.0, 8.0),
            Rect::new(0.0, 0.0, 16.0, 8.0),
        )
        .expect("record offset Picture blit");
    assert_eq!(engine.scratch_surface_size(), (1, 1));

    let encoder = engine.finish_recording().expect("finish recorder");
    assert!(encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::Native {
            operation: FrameRasterOp::FillRect { rect, .. }
        } if *rect == FrameRect::new(3, 2, 2, 2)
    )));
    assert!(!encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::PictureBlit { .. } | FrameCommand::CpuSegment { .. }
    )));
    let glyphs = encoder
        .commands()
        .iter()
        .find_map(|command| match command {
            FrameCommand::Native {
                operation: FrameRasterOp::BlitGlyphs { glyphs, .. },
            } => Some(glyphs),
            _ => None,
        })
        .expect("offset Picture glyphs stay native");
    assert_eq!((glyphs[0].x(), glyphs[0].y()), (8, 3));
    assert!(Arc::ptr_eq(glyphs[0].coverage(), &coverage));
    assert_eq!(
        encoder.render_reference().pixel(3, 2),
        Some(Color::green().premultiplied())
    );
}

#[test]
fn fractional_offset_picture_keeps_exact_cpu_fallback() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(24, 16).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    let picture = engine.create_offscreen(16, 8).expect("Picture");
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
    engine.canvas_2d().set_offset(0.5, 0.0);

    engine
        .try_blit_offscreen_src(
            &picture,
            Rect::new(0.0, 0.0, 16.0, 8.0),
            Rect::new(0.0, 0.0, 16.0, 8.0),
        )
        .expect("record fractional-offset Picture blit");
    assert_eq!(engine.scratch_surface_size(), (24, 16));

    let encoder = engine.finish_recording().expect("finish recorder");
    assert!(encoder
        .commands()
        .iter()
        .any(|command| matches!(command, FrameCommand::CpuSegment { .. })));
}

#[test]
fn recorder_pool_preserves_nested_blit_encoded_picture_and_memory_semantics() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(1200, 800).expect("initialize recorder");
    let placeholder_bytes = std::mem::size_of::<u32>();
    assert_eq!(engine.memory_usage(), placeholder_bytes);

    let source = engine.create_offscreen(2, 2).expect("source Picture");
    engine
        .try_begin_offscreen_paint(&source)
        .expect("begin source Picture");
    engine
        .offscreen_canvas(&source)
        .expect("source canvas")
        .fill_rect(Rect::new(0.0, 0.0, 2.0, 2.0), Color::red(), None);
    engine
        .try_flush_offscreen_paint(&source)
        .expect("flush source Picture");
    engine
        .try_end_offscreen_paint()
        .expect("end source Picture");

    let target = engine.create_offscreen(4, 4).expect("target Picture");
    engine
        .try_begin_offscreen_paint(&target)
        .expect("begin target Picture");
    engine
        .try_blit_offscreen_src(
            &source,
            Rect::new(0.0, 0.0, 2.0, 2.0),
            Rect::new(1.0, 1.0, 2.0, 2.0),
        )
        .expect("nested Picture blit");
    engine
        .try_flush_offscreen_paint(&target)
        .expect("flush target Picture");
    engine
        .try_end_offscreen_paint()
        .expect("end target Picture");
    let (pixels, width) = engine
        .copy_offscreen_pixels(&target)
        .expect("target pixels after nested blit");
    assert_eq!(width, 4);
    assert_eq!(pixels[5], Color::red().premultiplied());

    engine
        .try_begin_offscreen_paint(&target)
        .expect("begin failed target repaint");
    let self_blit = engine
        .try_blit_offscreen_src(
            &target,
            Rect::new(0.0, 0.0, 1.0, 1.0),
            Rect::new(0.0, 0.0, 1.0, 1.0),
        )
        .expect_err("a Picture cannot blit into itself");
    assert_eq!(self_blit.code(), Errc::InvalidArgument);
    engine
        .try_end_offscreen_paint()
        .expect("failed repaint aborts without replacing committed content");
    let (pixels, _) = engine
        .copy_offscreen_pixels(&target)
        .expect("failed repaint must retain old target pixels");
    assert_eq!(pixels[5], Color::red().premultiplied());
    assert!(
        engine.memory_usage()
            <= placeholder_bytes * 3 + (2 * 2 + 4 * 4) * std::mem::size_of::<u32>(),
        "inactive Picture recorders retain at most one 1x1 scratch plus the former BGRA payload"
    );

    let mut encoder = FrameEncoder::new(4, 4).expect("Picture encoder");
    encoder.clear(Color::blue());
    engine
        .try_begin_offscreen_paint(&target)
        .expect("begin encoded target repaint");
    assert!(matches!(
        engine
            .try_execute_encoded_picture(&target, &encoder)
            .expect("execute Picture encoder"),
        EncodedPictureExecution::Executed
    ));
    engine
        .try_end_offscreen_paint()
        .expect("commit encoded target repaint");
    let (pixels, _) = engine
        .copy_offscreen_pixels(&target)
        .expect("target pixels after encoded execution");
    assert!(pixels
        .iter()
        .all(|pixel| *pixel == Color::blue().premultiplied()));

    engine.destroy_offscreen(source);
    engine.destroy_offscreen(target);
    assert_eq!(
        engine.memory_usage(),
        placeholder_bytes,
        "destroyed Pictures must release pool pixels without retaining a main surface"
    );
}

#[test]
fn recorder_picture_begin_resets_state_and_end_propagates_deferred_error() {
    use crate::draw::primitives::path::PathBuilder;

    let mut engine = FrameRecordingEngine::new();
    engine.initialize(8, 6).expect("initialize recorder");
    let picture = engine.create_offscreen(4, 3).expect("Picture");
    engine
        .try_begin_offscreen_paint(&picture)
        .expect("begin first Picture paint");
    {
        let canvas = engine.offscreen_canvas(&picture).expect("Picture canvas");
        canvas.save();
        canvas.set_transform(Transform::translate(2.0, 1.0));
        canvas.push_clip(Rect::new(0.0, 0.0, 1.0, 1.0));
        canvas.set_opacity(0.5);
        canvas.push_clip_path(&PathBuilder::new().build());
    }
    let error = engine
        .try_end_offscreen_paint()
        .expect_err("end must surface an unflushed deferred error");
    assert_eq!(error.code(), Errc::NotImplemented);

    engine
        .try_begin_offscreen_paint(&picture)
        .expect("begin reused Picture paint");
    {
        let canvas = engine.offscreen_canvas(&picture).expect("reused canvas");
        canvas.restore();
        assert!(canvas.current_transform().is_identity());
        assert_eq!(canvas.offset(), (0.0, 0.0));
        assert_eq!(canvas.opacity(), 1.0);
        assert_eq!(canvas.current_clip(), Rect::new(0.0, 0.0, 4.0, 3.0));
        canvas.fill_rect(Rect::new(0.0, 0.0, 4.0, 3.0), Color::green(), None);
    }
    engine
        .try_flush_offscreen_paint(&picture)
        .expect("flush reused Picture paint");
    engine
        .try_end_offscreen_paint()
        .expect("end reused Picture paint");
    assert!(engine
        .copy_offscreen_pixels(&picture)
        .expect("reused Picture pixels")
        .0
        .iter()
        .all(|pixel| *pixel == Color::green().premultiplied()));

    engine
        .try_begin_offscreen_paint(&picture)
        .expect("begin failing repaint over committed Picture");
    {
        let canvas = engine.offscreen_canvas(&picture).expect("Picture canvas");
        canvas.fill_rect(Rect::new(0.0, 0.0, 4.0, 3.0), Color::red(), None);
        canvas.push_clip_path(&PathBuilder::new().build());
    }
    let error = engine
        .try_end_offscreen_paint()
        .expect_err("failed repaint must abort its candidate stream");
    assert_eq!(error.code(), Errc::NotImplemented);
    assert!(engine
        .copy_offscreen_pixels(&picture)
        .expect("old committed Picture survives failed repaint")
        .0
        .iter()
        .all(|pixel| *pixel == Color::green().premultiplied()));
    assert_eq!(
        engine.offscreen_scratch_surface_size(&picture),
        Some((1, 1))
    );
}

#[test]
fn direct_pixel_access_allocates_lazy_scratch() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(4, 3).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    let pixels = engine.canvas_2d().pixels_mut();
    assert_eq!(pixels.len(), 12);
    pixels[5] = Color::blue().premultiplied();
    assert_eq!(engine.scratch_surface_size(), (4, 3));

    let encoder = engine.finish_recording().expect("finish recorder");
    assert_eq!(
        encoder.render_reference().pixel(1, 1),
        Some(Color::blue().premultiplied())
    );
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
fn clipped_unscaled_picture_splices_native_ir_without_materializing() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(16, 10).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    let picture = engine.create_offscreen(16, 8).expect("Picture");
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
            Rect::new(0.0, 0.0, 16.0, 8.0),
            Rect::new(2.0, 1.0, 16.0, 8.0),
        )
        .expect("record clipped Picture blit");
    engine.canvas_2d().pop_clip();

    let encoder = engine.finish_recording().expect("finish recorder");
    assert!(encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::Native {
            operation: FrameRasterOp::FillRect { rect, .. }
        } if *rect == FrameRect::new(4, 2, 3, 2)
    )));
    assert!(!encoder.commands().iter().any(|command| matches!(
        command,
        FrameCommand::PictureBlit { .. } | FrameCommand::CpuSegment { .. }
    )));
    assert_eq!(engine.scratch_surface_size(), (1, 1));
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
        .push_clip(Rect::new(700.25, 502.0, 4.75, 4.0));
    engine
        .canvas_2d()
        .fill_rect(Rect::new(701.0, 503.0, 3.0, 2.0), color, None);
    engine.canvas_2d().pop_clip();

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
fn full_surface_translucent_rect_records_native_src_over() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(8, 6).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    engine
        .canvas_2d()
        .fill_rect(Rect::new(0.0, 0.0, 8.0, 6.0), Color::white(), None);
    let mask = Color::from_rgba(0, 0, 0, 128);
    engine
        .canvas_2d()
        .fill_rect(Rect::new(0.0, 0.0, 8.0, 6.0), mask, None);

    let encoder = engine
        .finish_recording()
        .expect("translucent full-surface fill must remain recordable");
    assert_eq!(
        encoder
            .commands()
            .iter()
            .filter(|command| matches!(command, FrameCommand::CpuSegment { .. }))
            .count(),
        0,
        "an axis-aligned translucent rectangle must not allocate a CPU image segment"
    );
    assert!(encoder.commands().iter().any(|command| {
        matches!(
            command,
            FrameCommand::Native {
                operation: FrameRasterOp::FillRect { rect, color }
            } if *rect == FrameRect::new(0, 0, 8, 6) && *color == mask
        )
    }));
    assert!(
        encoder
            .render_reference()
            .pixels()
            .iter()
            .all(|pixel| *pixel == 0xFF7F_7F7F),
        "native translucent fill must preserve exact SrcOver pixels"
    );
}

#[test]
fn consecutive_cpu_draws_batch_into_one_segment_until_barrier() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(32, 16).expect("initialize recorder");
    engine.begin_recording(true).expect("begin recording");
    // Two translucent CPU-only ellipse fills, then an opaque native barrier.
    engine.canvas_2d().fill_ellipse(
        Rect::new(2.0, 2.0, 4.0, 4.0),
        Color::from_rgba(255, 0, 0, 128),
    );
    engine.canvas_2d().fill_ellipse(
        Rect::new(8.0, 2.0, 4.0, 4.0),
        Color::from_rgba(0, 255, 0, 128),
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
    engine
        .canvas_2d()
        .fill_ellipse(Rect::new(8.0, 2.0, 4.0, 4.0), first);
    engine
        .canvas_2d()
        .fill_rect(Rect::new(32.0, 2.0, 2.0, 2.0), Color::white(), None);
    engine.canvas_2d().fill_ellipse(
        Rect::new(1.0, 2.0, 3.0, 3.0),
        Color::from_rgba(0, 255, 0, 128),
    );
    engine.canvas_2d().fill_ellipse(
        Rect::new(20.0, 2.0, 3.0, 3.0),
        Color::from_rgba(0, 0, 255, 128),
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

#[test]
fn oversized_resize_is_typed_and_keeps_previous_recording_extent() {
    let mut engine = FrameRecordingEngine::new();
    engine.initialize(8, 6).expect("initialize recorder");

    let error = engine
        .resize(i32::MAX, i32::MAX)
        .expect_err("oversized resize must fail");
    assert_eq!(error.code(), Errc::GraphicsOutOfMemory);

    engine
        .begin_recording(true)
        .expect("previous extent remains usable");
    let encoder = engine.finish_recording().expect("finish recorder");
    assert_eq!((encoder.width(), encoder.height()), (8, 6));
}
