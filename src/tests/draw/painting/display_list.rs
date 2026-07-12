use super::*;
use crate::draw::NullEngine;
use crate::draw::compositor::picture::encode_cached_picture;
use crate::draw::font::font_service::FontService;
use crate::draw::painting::PaintContext;
use crate::draw::primitives::path::{FillRule, PathBuilder};
use crate::draw::primitives::stroker::StrokeOptions;
use crate::draw::spatial::Orientation;
use crate::draw::traits::{GraphicsEngine, UpdateStrategy};
use crate::draw::{Color, FontHandle, SoftwareEngine};
use crate::ui::theme::DesignTokens;

const RED_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0,
    5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[test]
fn display_list_stores_ops() {
    let mut list = DisplayList::new();
    list.push(PaintOp::FillRect {
        rect: Rect::new(1.0, 2.0, 10.0, 10.0),
        color: Color::red(),
        radius: None,
    });
    assert_eq!(list.len(), 1);
    assert!(!list.is_empty());
}

#[test]
fn display_list_stores_draw_text_in_frame_and_set_font() {
    let mut list = DisplayList::new();
    list.push(PaintOp::SetFont {
        font: FontHandle::new(1),
    });
    list.push(PaintOp::DrawTextInFrame {
        text: "Nav".into(),
        rect: Rect::new(0.0, 0.0, 40.0, 20.0),
        color: Color::black(),
        font_size: 14.0,
    });
    assert_eq!(list.len(), 2);
}

#[test]
fn complete_picture_encoder_preserves_local_painter_order() {
    let mut list = DisplayList::new();
    list.push(PaintOp::Save);
    list.push(PaintOp::FillRect {
        rect: Rect::new(10.0, 20.0, 4.0, 3.0),
        color: Color::from_rgba(255, 0, 0, 128),
        radius: None,
    });
    list.push(PaintOp::FillRect {
        rect: Rect::new(12.0, 21.0, 3.0, 2.0),
        color: Color::from_rgba(0, 0, 255, 128),
        radius: None,
    });
    list.push(PaintOp::Restore);

    let fonts = FontService::new();
    let images = ImageService::new();
    let encoder = encode_cached_picture(
        &list,
        6,
        5,
        Point::new(10.0, 20.0),
        FontHandle::default(),
        &fonts,
        &images,
    )
    .expect("complete cached Picture");
    let frame = encoder.render_reference();

    assert_eq!(
        frame.pixel(0, 0),
        Some(Color::from_rgba(255, 0, 0, 128).premultiplied())
    );
    assert_eq!(
        frame.pixel(2, 1),
        Some(crate::draw::rasterizer::core::blend_srcover(
            128, 128, 0, 0, 128, 128, 0, 0,
        ))
    );
    assert_eq!(
        frame.pixel(5, 4),
        Some(Color::transparent().premultiplied())
    );
}

#[test]
fn complete_picture_encoder_preserves_clip_and_rounded_operations() {
    let mut clip = DisplayList::new();
    clip.push(PaintOp::PushClip {
        rect: Rect::new(0.0, 0.0, 2.0, 2.0),
    });
    clip.push(PaintOp::FillRect {
        rect: Rect::new(0.0, 0.0, 4.0, 4.0),
        color: Color::red(),
        radius: None,
    });
    clip.push(PaintOp::PopClip);

    let mut rounded = DisplayList::new();
    rounded.push(PaintOp::FillRect {
        rect: Rect::new(0.0, 0.0, 4.0, 4.0),
        color: Color::red(),
        radius: Some(crate::draw::Radius::uniform(2.0)),
    });
    let fonts = FontService::new();
    let images = ImageService::new();
    let clip_frame = encode_cached_picture(
        &clip,
        4,
        4,
        Point::new(0.0, 0.0),
        FontHandle::default(),
        &fonts,
        &images,
    )
    .expect("clip Picture must be encoded")
    .render_reference();
    let rounded_frame = encode_cached_picture(
        &rounded,
        4,
        4,
        Point::new(0.0, 0.0),
        FontHandle::default(),
        &fonts,
        &images,
    )
    .expect("rounded Picture must be encoded")
    .render_reference();

    assert_eq!(clip_frame.pixel(1, 1), Some(Color::red().premultiplied()));
    assert_eq!(
        clip_frame.pixel(3, 3),
        Some(Color::transparent().premultiplied())
    );
    assert_eq!(
        rounded_frame.pixel(2, 2),
        Some(Color::red().premultiplied())
    );
}

#[test]
fn replay_canvas_draw_image_with_service() {
    let svc = ImageService::new();
    let handle = svc.load_from_bytes(RED_PNG).expect("load png");

    let mut list = DisplayList::new();
    list.push(PaintOp::DrawImage {
        handle,
        bounds: Rect::new(0.0, 0.0, 4.0, 4.0),
        fit: true,
    });

    let mut engine = NullEngine::new();
    engine.initialize(8, 8).expect("init");
    let canvas = engine.canvas_2d();
    list.replay_canvas(
        canvas,
        Default::default(),
        &FontService::new(),
        Some(&svc),
        8.0,
    );
}

#[test]
fn display_list_replay_matches_recorded_extended_cpu_paint_pixels() {
    fn paint_extended(ctx: &mut PaintContext<'_>) {
        let path = PathBuilder::new()
            .move_to(2.0, 2.0)
            .line_to(20.0, 4.0)
            .line_to(4.0, 18.0)
            .close()
            .build();
        ctx.fill_ellipse(
            Rect::new(3.0, 3.0, 15.0, 10.0),
            Color::from_rgba(200, 20, 40, 180),
        );
        ctx.fill_sector(
            16.0,
            16.0,
            8.0,
            0.0,
            2.2,
            Color::from_rgba(40, 160, 220, 180),
        );
        ctx.fill_path(&path, Color::from_rgba(20, 220, 90, 160), FillRule::NonZero);
        ctx.stroke_circle(16.0, 16.0, 7.0, Color::white(), 2.0);
        ctx.stroke_path(&path, Color::black(), &StrokeOptions::default());
        ctx.draw_line(
            1.0,
            30.0,
            28.0,
            21.0,
            Color::from_rgba(220, 180, 20, 200),
            2.0,
        );
        ctx.fill_radial_gradient(25.0, 24.0, 1.0, 7.0, Color::white(), Color::black());
        ctx.draw_box_shadow_ambient(
            Rect::new(8.0, 20.0, 10.0, 8.0),
            2.0,
            1.0,
            1.0,
            Color::from_rgba(0, 0, 0, 120),
            None,
        );
    }

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let tokens = DesignTokens::antd_light();

    let mut recorded = SoftwareEngine::new();
    recorded.initialize(32, 32).expect("recorded init");
    let _ = recorded.begin_frame(UpdateStrategy::FullRedraw);
    let list = {
        let mut list = DisplayList::new();
        let mut ctx = PaintContext::new(
            recorded.canvas_2d(),
            FontHandle::default(),
            &font_service,
            &image_service,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            32,
            32,
        );
        ctx.set_recorder(Some(&mut list));
        paint_extended(&mut ctx);
        assert!(ctx.recording_complete());
        list
    };
    let _ = recorded.end_frame(&crate::draw::DamageRegion::full());
    let expected = recorded
        .session()
        .cpu_backend()
        .expect("CPU backend")
        .pixels()
        .to_vec();

    let mut replayed = SoftwareEngine::new();
    replayed.initialize(32, 32).expect("replayed init");
    let _ = replayed.begin_frame(UpdateStrategy::FullRedraw);
    {
        let mut ctx = PaintContext::new(
            replayed.canvas_2d(),
            FontHandle::default(),
            &font_service,
            &image_service,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            32,
            32,
        );
        list.replay(&mut ctx);
    }
    let _ = replayed.end_frame(&crate::draw::DamageRegion::full());
    let actual = replayed
        .session()
        .cpu_backend()
        .expect("CPU backend")
        .pixels()
        .to_vec();

    assert_eq!(
        actual, expected,
        "replay must preserve painter-order pixels"
    );
}
