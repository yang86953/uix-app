use crate::draw::pipeline::frame_encoder::*;
use crate::tests::common::*;
use std::sync::Arc;

fn rect(x: i32, y: i32, width: i32, height: i32, color: Color) -> FrameRasterOp {
    FrameRasterOp::FillRect {
        rect: FrameRect::new(x, y, width, height),
        color,
    }
}

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
fn full_picture_after_transparent_clear_restores_exact_premultiplied_pixels() {
    let source = vec![
        Color::from_rgba(200, 40, 20, 128).premultiplied(),
        Color::transparent().premultiplied(),
        Color::from_rgba(10, 220, 80, 64).premultiplied(),
        Color::white().premultiplied(),
    ];
    let mut encoder = FrameEncoder::new(2, 2).unwrap();
    encoder.clear(Color::transparent());
    encoder.blit_picture(
        FrameImage::new(2, 2, source.clone()).unwrap(),
        FrameRect::new(0, 0, 2, 2),
        FrameRect::new(0, 0, 2, 2),
    );

    assert_eq!(encoder.render_reference().pixels(), source);
}

#[test]
fn picture_opacity_matches_cpu_post_composition_channel_quantization() {
    let opacity = 0.37;
    let source = vec![
        Color::from_rgba(200, 40, 20, 128).premultiplied(),
        Color::transparent().premultiplied(),
        Color::from_rgba(10, 220, 80, 64).premultiplied(),
        Color::white().premultiplied(),
    ];
    let image = FrameImage::new(2, 2, source.clone()).unwrap();
    let mut encoder = FrameEncoder::new(4, 4).unwrap();
    let background = Color::from_rgb(12, 24, 48);
    encoder.clear(background);
    encoder.blit_picture_with_opacity(
        image,
        FrameRect::new(0, 0, 2, 2),
        FrameRect::new(1, 1, 2, 2),
        FrameOpacity::from_canvas(opacity),
    );

    let mut expected = vec![background.premultiplied(); 16];
    crate::draw::rasterizer::image::blit_image(
        &mut expected,
        4,
        4,
        Rect::new(0.0, 0.0, 4.0, 4.0),
        opacity,
        &source,
        2,
        Rect::new(0.0, 0.0, 2.0, 2.0),
        Rect::new(1.0, 1.0, 2.0, 2.0),
    );
    assert_eq!(encoder.render_reference().pixels(), expected);
    assert!(matches!(
        &encoder.commands()[1],
        FrameCommand::PictureBlit { opacity: actual, .. }
            if *actual == FrameOpacity::from_canvas(opacity)
    ));
}

#[test]
fn image_command_reference_tiles_match_full_frame_without_allocating_its_extent() {
    let source = (0..16)
        .map(|index| {
            Color::from_rgba(
                20 + index * 7,
                220 - index * 5,
                40 + index * 3,
                96 + index * 9,
            )
            .premultiplied()
        })
        .collect::<Vec<_>>();
    let image = FrameImage::new(4, 4, source).unwrap();
    let src = FrameRect::new(0, 0, 4, 4);
    let dst = FrameRect::new(-2, 5, 8, 8);
    let encoder = FrameEncoder::new(100, 80).unwrap();

    let (picture_tile, picture_bounds) = encoder
        .picture_blit_reference_tile(&image, src, dst, FrameOpacity::opaque())
        .expect("partially visible Picture tile");
    let (segment_tile, segment_bounds) = encoder
        .cpu_segment_reference_tile(&image, src, dst)
        .expect("partially visible CPU segment tile");
    assert_eq!(picture_bounds, FrameRect::new(0, 5, 6, 8));
    assert_eq!(segment_bounds, picture_bounds);
    assert_eq!((picture_tile.width(), picture_tile.height()), (6, 8));
    assert_eq!(segment_tile, picture_tile);

    let mut full = FrameEncoder::new(100, 80).unwrap();
    full.blit_picture(image, src, dst);
    let full = full.render_reference();
    for y in 0..picture_tile.height() {
        for x in 0..picture_tile.width() {
            assert_eq!(
                picture_tile.pixel(x, y),
                full.pixel(picture_bounds.x + x, picture_bounds.y + y),
                "tight tile must preserve scaled sampling at {x},{y}"
            );
        }
    }
}

#[test]
fn picture_splice_accepts_disjoint_src_over_and_rejects_quantizing_overlap() {
    let mut disjoint = FrameEncoder::new(16, 8).unwrap();
    disjoint.clear(Color::transparent());
    disjoint.native(rect(1, 1, 3, 3, Color::from_rgba(220, 40, 20, 128)));
    disjoint.native(rect(8, 1, 3, 3, Color::from_rgba(20, 180, 220, 160)));
    let translated = disjoint
        .translated_source_over_commands(4, 5, 32, 24)
        .expect("disjoint transparent SrcOver commands are exactly spliceable");
    assert!(matches!(
        translated.as_slice(),
        [
            FrameCommand::Native {
                operation: FrameRasterOp::FillRect { rect: first, .. }
            },
            FrameCommand::Native {
                operation: FrameRasterOp::FillRect { rect: second, .. }
            }
        ] if *first == FrameRect::new(5, 6, 3, 3)
            && *second == FrameRect::new(12, 6, 3, 3)
    ));

    let mut overlapping = FrameEncoder::new(16, 8).unwrap();
    overlapping.clear(Color::transparent());
    overlapping.native(rect(1, 1, 6, 4, Color::from_rgba(220, 40, 20, 128)));
    overlapping.native(rect(4, 2, 6, 4, Color::from_rgba(20, 180, 220, 160)));
    assert!(
        overlapping
            .translated_source_over_commands(4, 5, 32, 24)
            .is_none(),
        "8-bit intermediate SrcOver overlap must stay materialized because rounding is not associative"
    );
}

#[test]
fn picture_splice_accepts_overlapping_writes_inside_an_opaque_rect() {
    let mut source = FrameEncoder::new(16, 8).unwrap();
    source.clear(Color::transparent());
    source.native(rect(0, 0, 16, 8, Color::from_rgb(24, 48, 72)));
    let coverage: Arc<[u8]> = vec![0, 96, 192, 255, 255, 192, 96, 0].into();
    source.native(FrameRasterOp::BlitGlyphs {
        glyphs: vec![
            FrameGlyphBlit::new(
                3,
                2,
                Arc::clone(&coverage),
                4,
                2,
                Color::from_rgba(240, 80, 40, 192),
            )
            .unwrap(),
            FrameGlyphBlit::new(
                5,
                2,
                Arc::clone(&coverage),
                4,
                2,
                Color::from_rgba(40, 200, 240, 160),
            )
            .unwrap(),
        ],
        clip: FrameRect::new(0, 0, 16, 8),
    });
    source.native(rect(4, 1, 4, 4, Color::from_rgba(220, 200, 40, 96)));

    let translated = source
        .translated_source_over_commands(4, 3, 24, 16)
        .expect("opaque Picture background makes covered group overlap exact");
    let translated_glyphs = translated
        .iter()
        .find_map(|command| match command {
            FrameCommand::Native {
                operation: FrameRasterOp::BlitGlyphs { glyphs, .. },
            } => Some(glyphs),
            _ => None,
        })
        .expect("covered glyphs stay in the translated command stream");
    assert!(Arc::ptr_eq(translated_glyphs[0].coverage(), &coverage));
    assert!(Arc::ptr_eq(translated_glyphs[1].coverage(), &coverage));

    let parent_background = Color::from_rgb(96, 24, 12);
    let mut direct = FrameEncoder::new(24, 16).unwrap();
    direct.clear(parent_background);
    direct.append_validated_commands(translated).unwrap();

    let mut materialized = FrameEncoder::new(24, 16).unwrap();
    materialized.clear(parent_background);
    materialized.blit_picture(
        source.render_image(),
        FrameRect::new(0, 0, 16, 8),
        FrameRect::new(4, 3, 16, 8),
    );
    assert_eq!(
        direct.render_reference().pixels(),
        materialized.render_reference().pixels(),
        "opaque-backed grouping must be bit-exact, not tolerance-based"
    );

    let mut partial_barrier = FrameEncoder::new(16, 8).unwrap();
    partial_barrier.clear(Color::transparent());
    partial_barrier.native(rect(0, 0, 4, 8, Color::black()));
    partial_barrier.native(rect(2, 1, 8, 4, Color::from_rgba(220, 40, 20, 128)));
    partial_barrier.native(rect(6, 2, 6, 4, Color::from_rgba(20, 180, 220, 160)));
    assert!(
        partial_barrier
            .translated_source_over_commands(0, 0, 16, 8)
            .is_none(),
        "an opaque rect that does not fully cover either overlapping write is not a grouping proof"
    );
}

#[test]
fn picture_splice_accepts_only_fully_covered_overlap_from_opaque_rect_union() {
    let build_source = |include_bottom_right: bool| {
        let mut source = FrameEncoder::new(16, 8).unwrap();
        source.clear(Color::transparent());
        for cover in [
            rect(7, 1, 1, 2, Color::from_rgb(24, 48, 72)),
            rect(8, 1, 1, 2, Color::from_rgb(48, 72, 24)),
            rect(7, 3, 1, 2, Color::from_rgb(72, 24, 48)),
        ] {
            source.native(cover);
        }
        if include_bottom_right {
            source.native(rect(8, 3, 1, 2, Color::from_rgb(72, 48, 24)));
        }
        source.native(rect(2, 1, 7, 4, Color::from_rgba(220, 40, 20, 128)));
        source.native(rect(7, 1, 7, 4, Color::from_rgba(20, 180, 220, 160)));
        source
    };

    let source = build_source(true);
    let translated = source
        .translated_source_over_commands(3, 2, 24, 16)
        .expect("four opaque tiles jointly cover only the two translucent writes' overlap");
    let parent_background = Color::from_rgb(96, 24, 12);
    let mut direct = FrameEncoder::new(24, 16).unwrap();
    direct.clear(parent_background);
    direct.append_validated_commands(translated).unwrap();
    let mut materialized = FrameEncoder::new(24, 16).unwrap();
    materialized.clear(parent_background);
    materialized.blit_picture(
        source.render_image(),
        FrameRect::new(0, 0, 16, 8),
        FrameRect::new(3, 2, 16, 8),
    );
    assert_eq!(
        direct.render_reference().pixels(),
        materialized.render_reference().pixels(),
        "a gap-free opaque union over the actual overlap must remain bit-exact"
    );

    assert!(
        build_source(false)
            .translated_source_over_commands(0, 0, 16, 8)
            .is_none(),
        "a single uncovered pixel tile in the overlap must keep materialized fallback"
    );
}

#[test]
fn picture_splice_uses_only_the_guaranteed_opaque_interior_of_a_rounded_rect() {
    let radius = FrameRadius::new(crate::draw::primitives::types::Radius {
        tl: 5.0,
        tr: 3.0,
        br: 4.0,
        bl: 2.0,
    })
    .unwrap();
    let coverage: Arc<[u8]> = vec![0, 96, 192, 255, 255, 192, 96, 0].into();
    let mut source = FrameEncoder::new(24, 16).unwrap();
    source.clear(Color::transparent());
    source.native(FrameRasterOp::FillRoundedRect {
        rect: FrameRect::new(0, 0, 24, 16),
        color: Color::from_rgb(24, 48, 72),
        radius,
    });
    source.native(FrameRasterOp::BlitGlyphs {
        glyphs: vec![
            FrameGlyphBlit::new(
                7,
                7,
                Arc::clone(&coverage),
                4,
                2,
                Color::from_rgba(240, 80, 40, 192),
            )
            .unwrap(),
            FrameGlyphBlit::new(
                9,
                7,
                Arc::clone(&coverage),
                4,
                2,
                Color::from_rgba(40, 200, 240, 160),
            )
            .unwrap(),
        ],
        clip: FrameRect::new(0, 0, 24, 16),
    });

    let translated = source
        .translated_source_over_commands(4, 3, 32, 24)
        .expect("glyphs inside the rounded rect's guaranteed opaque interior are spliceable");
    let source_image = source.render_image();
    for y in 5..12 {
        for x in 5..20 {
            let index = y * 24 + x;
            assert_eq!(
                source_image.pixels()[index] >> 24,
                255,
                "the conservative rounded inner rect must be fully opaque at ({x}, {y})"
            );
        }
    }
    let mut direct = FrameEncoder::new(32, 24).unwrap();
    direct.clear(Color::from_rgb(96, 24, 12));
    direct.append_validated_commands(translated).unwrap();
    let mut materialized = FrameEncoder::new(32, 24).unwrap();
    materialized.clear(Color::from_rgb(96, 24, 12));
    materialized.blit_picture(
        source_image,
        FrameRect::new(0, 0, 24, 16),
        FrameRect::new(4, 3, 24, 16),
    );
    assert_premultiplied_pixels_within_one(
        materialized.render_reference().pixels(),
        direct.render_reference().pixels(),
    );

    let mut corner_overlap = FrameEncoder::new(24, 16).unwrap();
    corner_overlap.clear(Color::transparent());
    corner_overlap.native(FrameRasterOp::FillRoundedRect {
        rect: FrameRect::new(0, 0, 24, 16),
        color: Color::from_rgb(24, 48, 72),
        radius,
    });
    corner_overlap.native(FrameRasterOp::BlitGlyphs {
        glyphs: vec![FrameGlyphBlit::new(1, 1, coverage, 4, 2, Color::white()).unwrap()],
        clip: FrameRect::new(0, 0, 24, 16),
    });
    assert!(
        corner_overlap
            .translated_source_over_commands(0, 0, 24, 16)
            .is_none(),
        "rounded antialiased corners must not be treated as opaque cover"
    );

    let mut clipped = FrameEncoder::new(24, 16).unwrap();
    clipped.clear(Color::transparent());
    clipped.native(FrameRasterOp::FillRoundedRectClipped {
        rect: FrameRect::new(0, 0, 24, 16),
        color: Color::from_rgb(24, 48, 72),
        radius,
        clip: FrameRect::new(4, 4, 16, 8),
    });
    clipped.native(FrameRasterOp::BlitGlyphs {
        glyphs: vec![FrameGlyphBlit::new(
            7,
            7,
            Arc::from([255, 192, 96, 0, 0, 96, 192, 255]),
            4,
            2,
            Color::white(),
        )
        .unwrap()],
        clip: FrameRect::new(4, 4, 16, 8),
    });
    assert!(
        clipped
            .translated_source_over_commands(0, 0, 24, 16)
            .is_some(),
        "a clipped rounded fill exposes only the intersection of clip and guaranteed opaque interior"
    );
}

#[test]
fn picture_splice_crop_preserves_established_source_over_parity() {
    let mut source = FrameEncoder::new(16, 8).unwrap();
    source.clear(Color::transparent());
    source.native(FrameRasterOp::FillRoundedRect {
        rect: FrameRect::new(2, 1, 4, 4),
        color: Color::from_rgba(220, 40, 20, 128),
        radius: FrameRadius::new(crate::draw::primitives::types::Radius::uniform(2.0)).unwrap(),
    });
    let coverage: Arc<[u8]> = vec![0, 96, 255, 255, 96, 0].into();
    source.native(FrameRasterOp::BlitGlyphs {
        glyphs: vec![
            FrameGlyphBlit::new(7, 1, Arc::clone(&coverage), 3, 2, Color::white()).unwrap(),
        ],
        clip: FrameRect::new(0, 0, 16, 8),
    });
    source.cpu_image_segment(
        FrameImage::solid(4, 2, Color::from_rgba(20, 180, 220, 160)).unwrap(),
        FrameRect::new(0, 0, 4, 2),
        FrameRect::new(10, 1, 4, 2),
    );
    source.blit_picture(
        FrameImage::solid(3, 2, Color::from_rgba(80, 220, 60, 192)).unwrap(),
        FrameRect::new(0, 0, 3, 2),
        FrameRect::new(6, 5, 3, 2),
    );

    let crop = FrameRect::new(4, 0, 8, 8);
    let translated = source
        .translated_source_over_commands_in(crop, 6, 4, 24, 16)
        .expect("integer 1:1 Picture crop must remain spliceable");
    assert!(translated.iter().any(|command| matches!(
        command,
        FrameCommand::Native {
            operation: FrameRasterOp::FillRoundedRectClipped { rect, clip, .. }
        } if *rect == FrameRect::new(8, 5, 4, 4)
            && *clip == FrameRect::new(10, 5, 2, 4)
    )));
    assert!(translated.iter().any(|command| matches!(
        command,
        FrameCommand::CpuSegment { src, dst, .. }
            if *src == FrameRect::new(0, 0, 2, 2)
                && *dst == FrameRect::new(16, 5, 2, 2)
    )));

    let background = Color::from_rgb(12, 24, 48);
    let mut direct = FrameEncoder::new(24, 16).unwrap();
    direct.clear(background);
    direct.append_validated_commands(translated).unwrap();

    let mut materialized = FrameEncoder::new(24, 16).unwrap();
    materialized.clear(background);
    materialized.blit_picture(source.render_image(), crop, FrameRect::new(10, 4, 8, 8));
    assert_premultiplied_pixels_within_one(
        materialized.render_reference().pixels(),
        direct.render_reference().pixels(),
    );

    let translated_at_edge = source
        .translated_source_over_commands_in(crop, -4, 0, 8, 8)
        .expect("cropped visible region fits even when original geometry crosses the target edge");
    assert!(translated_at_edge.iter().any(|command| matches!(
        command,
        FrameCommand::Native {
            operation: FrameRasterOp::FillRoundedRectClipped { rect, clip, .. }
        } if *rect == FrameRect::new(-2, 1, 4, 4)
            && *clip == FrameRect::new(0, 1, 2, 4)
    )));
    let mut direct_at_edge = FrameEncoder::new(8, 8).unwrap();
    direct_at_edge.clear(background);
    direct_at_edge
        .append_validated_commands(translated_at_edge)
        .unwrap();
    let mut materialized_at_edge = FrameEncoder::new(8, 8).unwrap();
    materialized_at_edge.clear(background);
    materialized_at_edge.blit_picture(source.render_image(), crop, FrameRect::new(0, 0, 8, 8));
    assert_premultiplied_pixels_within_one(
        materialized_at_edge.render_reference().pixels(),
        direct_at_edge.render_reference().pixels(),
    );

    assert!(source
        .translated_source_over_commands_in(FrameRect::new(15, 0, 2, 2), 0, 0, 24, 16)
        .is_none());
    assert!(source
        .translated_source_over_commands_in(crop, i32::MAX, 0, 24, 16)
        .is_none());
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
    encoder
        .cpu_segment([rect(1, 0, 3, 1, Color::green())])
        .unwrap();
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
    encoder.cpu_segment([rect(1, 1, 4, 3, second)]).unwrap();

    let mut cpu = SharedRasterizer::new(PixelSurface::new(4, 3));
    cpu.surface_mut().set_clear_color(clear);
    cpu.surface_mut().clear_all();
    cpu.fill_rect(Rect::new(-1.0, 0.0, 3.0, 3.0), first, None);
    cpu.fill_rect(Rect::new(1.0, 1.0, 4.0, 3.0), second, None);

    assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());
}

#[test]
fn cpu_segment_rejects_destination_dependent_operations_without_mutating_encoder() {
    let mut encoder = FrameEncoder::new(3, 2).unwrap();
    encoder.clear(Color::black());
    let command_count = encoder.commands().len();

    let additive = encoder
        .cpu_segment([FrameRasterOp::FillRectAdditive {
            rect: FrameRect::new(0, 0, 1, 1),
            color: Color::white(),
        }])
        .unwrap_err();
    assert_eq!(
        additive,
        FrameEncoderError::DestinationDependentCpuSegment {
            operation: "FillRectAdditive"
        }
    );
    assert_eq!(encoder.commands().len(), command_count);

    let rounded_additive = encoder
        .cpu_segment([FrameRasterOp::FillRoundedRectAdditive {
            rect: FrameRect::new(0, 0, 2, 2),
            color: Color::white(),
            radius: FrameRadius::new(crate::draw::primitives::types::Radius::uniform(1.0)).unwrap(),
        }])
        .unwrap_err();
    assert_eq!(
        rounded_additive,
        FrameEncoderError::DestinationDependentCpuSegment {
            operation: "FillRoundedRectAdditive"
        }
    );
    assert_eq!(encoder.commands().len(), command_count);

    let scroll = encoder
        .cpu_segment([FrameRasterOp::ScrollCopy {
            viewport: FrameRect::new(0, 0, 3, 2),
            dx: 1,
            dy: 0,
        }])
        .unwrap_err();
    assert_eq!(
        scroll,
        FrameEncoderError::DestinationDependentCpuSegment {
            operation: "ScrollCopy"
        }
    );
    assert_eq!(encoder.commands().len(), command_count);
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
fn rounded_additive_fill_matches_shared_cpu_sdf_and_destination_blend() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
    use crate::draw::primitives::types::{BlendMode, Radius};
    use crate::draw::traits::Canvas2D;

    let base = Color::from_rgba(36, 72, 108, 180);
    let add = Color::from_rgba(220, 80, 40, 144);
    let radius = Radius {
        tl: 3.0,
        tr: 2.0,
        br: 1.5,
        bl: 2.5,
    };
    let mut encoder = FrameEncoder::new(9, 7).unwrap();
    encoder.clear(base);
    encoder.native(FrameRasterOp::FillRoundedRectAdditive {
        rect: FrameRect::new(1, 1, 7, 5),
        color: add,
        radius: FrameRadius::new(radius).unwrap(),
    });

    let mut cpu = SharedRasterizer::new(PixelSurface::new(9, 7));
    cpu.surface_mut().set_clear_color(base);
    cpu.surface_mut().clear_all();
    cpu.set_blend_mode(BlendMode::Additive);
    cpu.fill_rect(Rect::new(1.0, 1.0, 7.0, 5.0), add, Some(radius));

    assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());
}

#[test]
fn rounded_src_over_fill_matches_shared_cpu_sdf() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
    use crate::draw::primitives::types::Radius;
    use crate::draw::traits::Canvas2D;

    let base = Color::from_rgba(36, 72, 108, 180);
    let fill = Color::from_rgba(220, 80, 40, 144);
    let radius = Radius {
        tl: 3.0,
        tr: 0.0,
        br: 6.0,
        bl: 2.5,
    };
    let mut encoder = FrameEncoder::new(11, 9).unwrap();
    encoder.clear(base);
    encoder.native(FrameRasterOp::FillRoundedRect {
        rect: FrameRect::new(1, 1, 9, 7),
        color: fill,
        radius: FrameRadius::new(radius).unwrap(),
    });

    let mut cpu = SharedRasterizer::new(PixelSurface::new(11, 9));
    cpu.surface_mut().set_clear_color(base);
    cpu.surface_mut().clear_all();
    cpu.fill_rect(Rect::new(1.0, 1.0, 9.0, 7.0), fill, Some(radius));

    assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());

    let mut segment = FrameEncoder::new(11, 9).unwrap();
    segment
        .cpu_segment([FrameRasterOp::FillRoundedRect {
            rect: FrameRect::new(1, 1, 9, 7),
            color: fill,
            radius: FrameRadius::new(radius).unwrap(),
        }])
        .expect("ordinary rounded fill is source-independent");
    assert!(matches!(
        segment.commands(),
        [FrameCommand::CpuSegment { .. }]
    ));
}

#[test]
fn clipped_rounded_src_over_matches_shared_cpu_sdf_and_transparent_segment() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
    use crate::draw::primitives::types::Radius;
    use crate::draw::traits::Canvas2D;

    let base = Color::from_rgba(36, 72, 108, 180);
    let fill = Color::from_rgba(220, 80, 40, 144);
    let radius = Radius {
        tl: 3.0,
        tr: 8.0,
        br: 20.0,
        bl: 0.0,
    };
    let operation = FrameRasterOp::FillRoundedRectClipped {
        rect: FrameRect::new(1, 1, 11, 8),
        color: fill,
        radius: FrameRadius::new(radius).unwrap(),
        clip: FrameRect::new(3, 2, 7, 5),
    };
    let mut encoder = FrameEncoder::new(14, 11).unwrap();
    encoder.clear(base);
    encoder.native(operation.clone());

    let mut cpu = SharedRasterizer::new(PixelSurface::new(14, 11));
    cpu.surface_mut().set_clear_color(base);
    cpu.surface_mut().clear_all();
    cpu.push_clip(Rect::new(3.0, 2.0, 7.0, 5.0));
    cpu.fill_rect(Rect::new(1.0, 1.0, 11.0, 8.0), fill, Some(radius));
    assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());

    let mut native_transparent = FrameEncoder::new(14, 11).unwrap();
    native_transparent.native(operation.clone());
    let mut segment = FrameEncoder::new(14, 11).unwrap();
    segment
        .cpu_segment([operation])
        .expect("clipped SrcOver rounded fill is source-independent");
    assert_eq!(
        segment.render_reference().pixels(),
        native_transparent.render_reference().pixels()
    );
}

#[test]
fn clipped_stroke_rect_ir_matches_shared_cpu_and_compact_reference_tile() {
    use crate::draw::engine::cpu::pixel_surface::PixelSurface;
    use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
    use crate::draw::primitives::types::Radius;
    use crate::draw::traits::Canvas2D;

    let base = Color::from_rgba(36, 72, 108, 180);
    let stroke = Color::from_rgba(220, 80, 40, 144);
    let radius = Radius {
        tl: 3.0,
        tr: 1.0,
        br: 4.0,
        bl: 0.0,
    };
    let rect = FrameRect::new(4, 3, 10, 7);
    let clip = FrameRect::new(2, 2, 13, 9);
    let line_width = FrameStrokeWidth::new(2.5).unwrap();
    let stroke_command =
        FrameStrokeRect::new(rect, stroke, FrameRadius::new(radius).unwrap(), line_width);
    let operation = FrameRasterOp::StrokeRoundedRects {
        strokes: vec![stroke_command],
        clip,
    };
    let mut encoder = FrameEncoder::new(18, 14).unwrap();
    encoder.clear(base);
    encoder.native(operation.clone());

    let mut cpu = SharedRasterizer::new(PixelSurface::new(18, 14));
    cpu.surface_mut().set_clear_color(base);
    cpu.surface_mut().clear_all();
    cpu.push_clip(Rect::new(2.0, 2.0, 13.0, 9.0));
    cpu.stroke_rect(Rect::new(4.0, 3.0, 10.0, 7.0), stroke, 2.5, Some(radius));
    assert_eq!(encoder.render_reference().pixels(), cpu.surface().pixels());

    let transparent = FrameEncoder::new(18, 14).unwrap();
    let (tile, destination) = transparent
        .stroke_rects_reference_tile(&[stroke_command], clip)
        .expect("allocate compact stroke tile")
        .expect("visible stroke tile");
    assert!(tile.width() < 18 || tile.height() < 14);
    let mut tiled = FrameEncoder::new(18, 14).unwrap();
    tiled.cpu_image_segment(
        FrameImage::new(tile.width(), tile.height(), tile.pixels().to_vec()).unwrap(),
        FrameRect::new(0, 0, tile.width(), tile.height()),
        destination,
    );
    let mut direct = FrameEncoder::new(18, 14).unwrap();
    direct.native(operation);
    assert_eq!(tiled.render_reference(), direct.render_reference());
}

#[test]
fn frame_stroke_width_rejects_zero_negative_and_non_finite_values() {
    for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            FrameStrokeWidth::new(invalid),
            Err(FrameEncoderError::InvalidStrokeWidth)
        );
    }
    assert_eq!(FrameStrokeWidth::new(2.5).unwrap().value(), 2.5);
}

#[test]
fn adjacent_strokes_batch_within_sparse_union_bounds_and_split_beyond_them() {
    let radius = FrameRadius::new(crate::draw::Radius::uniform(2.0)).unwrap();
    let line_width = FrameStrokeWidth::new(1.0).unwrap();
    let clip = FrameRect::new(0, 0, 2000, 100);
    let operation = |rect| FrameRasterOp::StrokeRoundedRects {
        strokes: vec![FrameStrokeRect::new(
            rect,
            Color::white(),
            radius,
            line_width,
        )],
        clip,
    };
    let mut encoder = FrameEncoder::new(2000, 100).unwrap();
    encoder.native(operation(FrameRect::new(10, 10, 20, 20)));
    encoder.native(operation(FrameRect::new(35, 10, 20, 20)));
    encoder.native(operation(FrameRect::new(1800, 10, 20, 20)));

    assert!(matches!(
        encoder.commands(),
        [
            FrameCommand::Native {
                operation: FrameRasterOp::StrokeRoundedRects { strokes: first, .. }
            },
            FrameCommand::Native {
                operation: FrameRasterOp::StrokeRoundedRects { strokes: second, .. }
            }
        ] if first.len() == 2 && second.len() == 1
    ));
    let mut control = FrameEncoder::new(2000, 100).unwrap();
    control.native(rect(0, 0, 1, 1, Color::white()));
    control.native(rect(2, 0, 1, 1, Color::white()));
    assert!(
        encoder.retained_memory_usage()
            >= control
                .retained_memory_usage()
                .saturating_add(3 * std::mem::size_of::<FrameStrokeRect>()),
        "Picture budget accounting must include retained stroke batch payloads"
    );

    let mut overlap = FrameEncoder::new(2000, 100).unwrap();
    overlap.native(FrameRasterOp::StrokeRoundedRects {
        strokes: vec![
            FrameStrokeRect::new(
                FrameRect::new(10, 10, 20, 20),
                Color::white(),
                radius,
                line_width,
            ),
            FrameStrokeRect::new(
                FrameRect::new(20, 10, 20, 20),
                Color::white(),
                radius,
                line_width,
            ),
        ],
        clip,
    });
    assert_eq!(
        overlap.commands().len(),
        2,
        "conservative overlap must preserve separate 8-bit SrcOver boundaries"
    );
}

#[test]
fn frame_radius_rejects_non_finite_and_negative_corners() {
    assert_eq!(
        FrameRadius::new(crate::draw::primitives::types::Radius {
            tl: f32::NAN,
            tr: 0.0,
            br: 0.0,
            bl: 0.0,
        }),
        Err(FrameEncoderError::InvalidRadius { corner: "top-left" })
    );
    assert_eq!(
        FrameRadius::new(crate::draw::primitives::types::Radius {
            tl: 0.0,
            tr: 0.0,
            br: -1.0,
            bl: 0.0,
        }),
        Err(FrameEncoderError::InvalidRadius {
            corner: "bottom-right"
        })
    );
    assert_eq!(
        FrameRadius::new(crate::draw::primitives::types::Radius {
            tl: -0.0,
            tr: 0.0,
            br: 0.0,
            bl: 0.0,
        })
        .unwrap()
        .to_radius()
        .tl
        .to_bits(),
        0.0f32.to_bits()
    );
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

#[test]
fn frame_glyph_blit_validates_payload_without_copying_shared_coverage() {
    let coverage: Arc<[u8]> = vec![0, 1, 127, 255, 12].into();
    let glyph = FrameGlyphBlit::new(3, 4, Arc::clone(&coverage), 2, 2, Color::white())
        .expect("trailing coverage bytes are harmless");
    assert!(Arc::ptr_eq(glyph.coverage(), &coverage));
    assert_eq!(
        (glyph.x(), glyph.y(), glyph.width(), glyph.height()),
        (3, 4, 2, 2)
    );

    for (width, height, actual) in [(0, 2, 0), (2, 0, 0), (2, 2, 3)] {
        let error =
            FrameGlyphBlit::new(0, 0, vec![0; actual].into(), width, height, Color::white())
                .unwrap_err();
        assert!(matches!(
            error,
            FrameEncoderError::InvalidGlyphCoverage { .. }
        ));
    }
    assert!(matches!(
        FrameGlyphBlit::new(0, 0, Arc::from([]), usize::MAX, 2, Color::white()),
        Err(FrameEncoderError::InvalidGlyphCoverage { .. })
    ));
}

#[test]
fn adjacent_same_clip_glyphs_batch_without_crossing_painter_barriers() {
    let glyph = |x| FrameGlyphBlit::new(x, 0, Arc::from([255]), 1, 1, Color::white()).unwrap();
    let clip = FrameRect::new(0, 0, 8, 2);
    let mut encoder = FrameEncoder::new(8, 2).unwrap();
    encoder.native(FrameRasterOp::BlitGlyphs {
        glyphs: vec![glyph(0)],
        clip,
    });
    encoder.native(FrameRasterOp::BlitGlyphs {
        glyphs: vec![glyph(1)],
        clip,
    });
    encoder.native(rect(3, 0, 1, 1, Color::red()));
    encoder.native(FrameRasterOp::BlitGlyphs {
        glyphs: vec![glyph(4)],
        clip,
    });

    assert!(matches!(
        encoder.commands(),
        [
            FrameCommand::Native {
                operation: FrameRasterOp::BlitGlyphs { glyphs, .. }
            },
            FrameCommand::Native {
                operation: FrameRasterOp::FillRect { .. }
            },
            FrameCommand::Native {
                operation: FrameRasterOp::BlitGlyphs { .. }
            }
        ] if glyphs.len() == 2
    ));
}

#[test]
fn glyph_reference_execution_matches_shared_cpu_rasterizer_with_clip_and_overlap() {
    let coverage: Arc<[u8]> = vec![0, 1, 127, 128, 254, 255, 64, 192].into();
    let color = Color::from_rgba(220, 80, 40, 144);
    let clip = FrameRect::new(2, 1, 4, 2);
    let mut encoder = FrameEncoder::new(8, 4).unwrap();
    encoder.clear(Color::from_rgba(30, 60, 90, 180));
    encoder.native(FrameRasterOp::BlitGlyphs {
        glyphs: vec![
            FrameGlyphBlit::new(1, 1, Arc::clone(&coverage), 4, 2, color).unwrap(),
            FrameGlyphBlit::new(3, 1, coverage, 4, 2, color).unwrap(),
        ],
        clip,
    });

    let mut expected = vec![Color::from_rgba(30, 60, 90, 180).premultiplied(); 8 * 4];
    let coverage = [0, 1, 127, 128, 254, 255, 64, 192];
    for x in [1, 3] {
        crate::draw::rasterizer::glyph::blit_glyph(
            &mut expected,
            8,
            4,
            Rect::new(2.0, 1.0, 4.0, 2.0),
            1.0,
            x,
            1,
            &coverage,
            4,
            2,
            color,
        );
    }
    assert_eq!(encoder.render_reference().pixels(), expected);

    let mut segment = FrameEncoder::new(8, 4).unwrap();
    segment
        .cpu_segment([FrameRasterOp::BlitGlyphs {
            glyphs: vec![FrameGlyphBlit::new(1, 1, Arc::from(coverage), 4, 2, color).unwrap()],
            clip,
        }])
        .expect("glyph batches are source-independent");
}
