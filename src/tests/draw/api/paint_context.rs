use crate::draw::api::paint_context::*;
use crate::draw::geometry::spatial::PhysicalUnit;
use crate::tests::common::*;

#[test]
fn resolve_font_size_no_unit_returns_base() {
    assert_eq!(resolve_font_size(14.0, None, 96.0), 14.0);
}

#[test]
fn resolve_font_size_zero_base() {
    assert_eq!(resolve_font_size(0.0, None, 96.0), 0.0);
}

#[test]
fn resolve_font_size_with_dip_unit_returns_base() {
    let unit = PhysicalUnit::Px(16.0);
    let result = resolve_font_size(14.0, Some(unit), 96.0);
    // Dip 直接返回其值
    assert_eq!(result, 16.0);
}

#[test]
fn resolve_font_size_with_mm_unit() {
    let unit = PhysicalUnit::Mm(10.0);
    // 10mm @ 96 DPI ≈ 37.795
    let result = resolve_font_size(14.0, Some(unit), 96.0);
    assert!((result - 37.795).abs() < 0.01);
}

#[test]
fn resolve_font_size_with_pt_unit() {
    let unit = PhysicalUnit::Pt(12.0);
    // 12pt @ 96 DPI = 12 * 96/72 = 16
    let result = resolve_font_size(14.0, Some(unit), 96.0);
    assert!((result - 16.0).abs() < 0.01);
}

#[test]
fn resolve_font_size_with_pt_unit_high_dpi() {
    let unit = PhysicalUnit::Pt(12.0);
    let result = resolve_font_size(14.0, Some(unit), 192.0);
    // 12pt @ 192 DPI = 12 * 192/72 = 32
    assert!((result - 32.0).abs() < 0.01);
}

#[test]
fn resolve_font_size_with_px_unit() {
    let unit = PhysicalUnit::Px(20.0);
    let result = resolve_font_size(14.0, Some(unit), 96.0);
    // px 直接返回其值
    assert_eq!(result, 20.0);
}

/// PaintContext 默认 debug=false；未 set_debug_mode(true) 时边框绘制为 no-op。
/// LayerTree::draw_debug_for_widget 必须先打开，否则 debug overlay 不可见。
#[test]
fn paint_context_debug_mode_defaults_off_until_enabled() {
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::geometry::spatial::Orientation;

    let mut canvas = NoopCanvas2D;
    let fs = FontService::new();
    let img = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        FontHandle::default(),
        &fs,
        &img,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        100,
        100,
    );
    assert!(!ctx.debug_mode());
    ctx.set_debug_mode(true);
    assert!(ctx.debug_mode());
    // 打开后边框路径可走通（NoopCanvas 不记录，仅防 panic）
    ctx.draw_debug_border(Rect::new(0.0, 0.0, 40.0, 20.0), 0, false);
}

#[test]
fn paint_context_queries_preserve_display_list_recording() {
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::command::DisplayList;
    use crate::draw::geometry::spatial::Orientation;

    let mut canvas = NoopCanvas2D;
    let fs = FontService::new();
    let img = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        FontHandle::default(),
        &fs,
        &img,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        100,
        100,
    );
    let mut list = DisplayList::new();
    ctx.with_recorder(&mut list, |ctx| {
        assert_eq!(ctx.surface_size(), Size::new(100.0, 100.0));
        assert!(ctx.is_rect_visible(Rect::new(0.0, 0.0, 0.5, 0.5)));
        assert!(!ctx.is_rect_visible(Rect::new(200.0, 200.0, 1.0, 1.0)));
        ctx.fill_rect(Rect::new(0.0, 0.0, 10.0, 10.0), Color::red(), None);
        assert!(ctx.recording_complete());
    });
    assert_eq!(list.len(), 1, "read-only queries must not emit paint ops");
}

#[test]
fn public_geometry_helpers_each_record_one_canonical_paint_op() {
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::command::DisplayList;
    use crate::draw::geometry::spatial::Orientation;
    use crate::draw::Radius;

    let mut canvas = NoopCanvas2D;
    let fs = FontService::new();
    let img = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        FontHandle::default(),
        &fs,
        &img,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        100,
        100,
    );
    let points = [
        Point::new(4.0, 4.0),
        Point::new(20.0, 6.0),
        Point::new(10.0, 22.0),
    ];
    let stroke = StrokeOptions {
        width: 2.0,
        ..StrokeOptions::default()
    };
    let mut list = DisplayList::new();

    ctx.with_recorder(&mut list, |ctx| {
        ctx.draw_point(Point::new(2.0, 2.0), Color::red(), 3.0)
    });
    assert_eq!(list.len(), 1);
    ctx.with_recorder(&mut list, |ctx| {
        ctx.fill_polygon(&points, Color::green(), FillRule::NonZero)
    });
    assert_eq!(list.len(), 2);
    ctx.with_recorder(&mut list, |ctx| {
        ctx.stroke_polyline(&points, Color::blue(), &stroke)
    });
    assert_eq!(list.len(), 3);
    ctx.with_recorder(&mut list, |ctx| {
        ctx.stroke_polygon(&points, Color::white(), &stroke)
    });
    assert_eq!(list.len(), 4);
    ctx.with_recorder(&mut list, |ctx| {
        ctx.stroke_ellipse(Rect::new(24.0, 4.0, 12.0, 8.0), Color::black(), &stroke)
    });
    assert_eq!(list.len(), 5);
    ctx.with_recorder(&mut list, |ctx| {
        ctx.fill_rounded_rect(
            Rect::new(4.0, 28.0, 16.0, 10.0),
            Color::red(),
            Radius::uniform(3.0),
        )
    });
    assert_eq!(list.len(), 6);
    ctx.with_recorder(&mut list, |ctx| {
        ctx.stroke_rounded_rect(
            Rect::new(24.0, 28.0, 16.0, 10.0),
            Color::blue(),
            2.0,
            Radius::uniform(3.0),
        )
    });
    assert_eq!(list.len(), 7);

    assert!(ctx.recording_complete());
    assert_eq!(
        list.len(),
        7,
        "each public geometry helper must lower to exactly one canonical PaintOp"
    );
}

#[test]
fn paint_context_state_helpers_record_and_restore_canonical_state() {
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::command::DisplayList;
    use crate::draw::geometry::path::PathBuilder;
    use crate::draw::geometry::spatial::Orientation;
    use crate::draw::{BlendMode, Transform};

    let mut canvas = NoopCanvas2D;
    let fs = FontService::new();
    let img = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        FontHandle::default(),
        &fs,
        &img,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        100,
        100,
    );
    let mut list = DisplayList::new();
    let clip_path = PathBuilder::new()
        .polygon(&[
            Point::new(0.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(20.0, 20.0),
            Point::new(0.0, 20.0),
        ])
        .build();

    ctx.with_recorder(&mut list, |ctx| {
        ctx.translate(2.0, 3.0);
        ctx.set_transform(Transform::translate(1.0, 1.0));
        ctx.concat_transform(Transform::scale(2.0, 2.0));
        ctx.set_blend_mode(BlendMode::SrcOver);
        ctx.set_opacity(0.75);
        ctx.push_clip_path(&clip_path);
        ctx.pop_clip();
        ctx.with_opacity(0.5, |ctx| {
            ctx.draw_point(Point::new(8.0, 8.0), Color::red(), 4.0);
        });
    });

    assert!(ctx.recording_complete());
    assert_eq!(
        list.len(),
        11,
        "transform, clip, blend and opacity state must all be recorded in painter order"
    );
}

#[test]
fn with_opacity_restores_canvas_state_after_panic() {
    use crate::draw::backend::cpu::pixel_surface::PixelSurface;
    use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
    use crate::draw::geometry::spatial::Orientation;
    use crate::draw::Canvas2D;

    let mut canvas = SharedRasterizer::new(PixelSurface::new(16, 16));
    let fs = FontService::new();
    let img = ImageService::new();
    let tokens = DesignTokens::antd_light();
    {
        let mut ctx = PaintContext::new_for_test(
            &mut canvas,
            FontHandle::default(),
            &fs,
            &img,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            16,
            16,
        );
        ctx.set_opacity(0.6);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ctx.with_opacity(0.5, |_| panic!("abort opacity scope"));
        }));
        assert!(result.is_err());
    }

    assert!((canvas.opacity() - 0.6).abs() < f32::EPSILON);
}

#[test]
fn display_list_recording_covers_extended_2d_paint_operations() {
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::command::DisplayList;
    use crate::draw::geometry::path::{FillRule, PathBuilder};
    use crate::draw::geometry::spatial::Orientation;
    use crate::draw::geometry::stroker::StrokeOptions;

    let mut canvas = NoopCanvas2D;
    let fs = FontService::new();
    let img = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        FontHandle::default(),
        &fs,
        &img,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        100,
        100,
    );
    let mut list = DisplayList::new();
    let path = PathBuilder::new()
        .move_to(1.0, 1.0)
        .line_to(8.0, 1.0)
        .line_to(1.0, 8.0)
        .close()
        .build();

    ctx.with_recorder(&mut list, |ctx| {
        ctx.fill_ellipse(Rect::new(1.0, 1.0, 8.0, 6.0), Color::red());
        ctx.fill_sector(5.0, 5.0, 3.0, 0.0, 1.0, Color::green());
        ctx.fill_path(&path, Color::blue(), FillRule::NonZero);
        ctx.stroke_circle(5.0, 5.0, 3.0, Color::white(), 1.0);
        ctx.stroke_arc(5.0, 5.0, 3.0, 0.0, 1.5, Color::white(), 1.0);
        ctx.stroke_path(&path, Color::black(), &StrokeOptions::default());
        ctx.draw_line(1.0, 1.0, 8.0, 8.0, Color::white(), 1.0);
        ctx.fill_radial_gradient(5.0, 5.0, 0.0, 4.0, Color::white(), Color::black());
        ctx.draw_box_shadow_ambient(
            Rect::new(1.0, 1.0, 6.0, 6.0),
            2.0,
            1.0,
            1.0,
            Color::black(),
            None,
        );
    });

    assert!(ctx.recording_complete());
    assert_eq!(list.len(), 9);
}

#[test]
fn text_ops_are_recorded_only_inside_recorder_scope() {
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::command::DisplayList;
    use crate::draw::geometry::spatial::Orientation;

    let mut canvas = NoopCanvas2D;
    let fonts = FontService::new();
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        FontHandle::default(),
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        100,
        100,
    );
    let mut list = DisplayList::new();

    ctx.draw_text("direct", Point::new(0.0, 0.0), Color::black(), 12.0);
    assert!(list.is_empty());

    ctx.with_recorder(&mut list, |ctx| {
        ctx.draw_text("plain", Point::new(0.0, 0.0), Color::black(), 12.0);
        ctx.draw_text_baseline("baseline", 0.0, 12.0, Color::black(), 12.0);
        ctx.text_center(
            "center",
            Rect::new(0.0, 0.0, 40.0, 20.0),
            Color::black(),
            12.0,
        );
        ctx.draw_text_in_frame(
            "frame",
            Rect::new(0.0, 0.0, 40.0, 20.0),
            Color::black(),
            12.0,
        );
        ctx.draw_text_wrapped(
            "wrapped",
            Rect::new(0.0, 0.0, 40.0, 20.0),
            Color::black(),
            12.0,
        );
        ctx.draw_text_with_selection(
            "selected",
            Point::new(0.0, 0.0),
            Color::black(),
            12.0,
            Some((0, 3)),
            Color::blue(),
        );
    });

    assert_eq!(list.len(), 6);
}

#[test]
fn nested_recorder_scope_restores_outer_target() {
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::command::DisplayList;
    use crate::draw::geometry::spatial::Orientation;

    let mut canvas = NoopCanvas2D;
    let fonts = FontService::new();
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        FontHandle::default(),
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        100,
        100,
    );
    let mut outer = DisplayList::new();
    let mut inner = DisplayList::new();

    ctx.with_recorder(&mut outer, |ctx| {
        ctx.fill_rect(Rect::new(0.0, 0.0, 2.0, 2.0), Color::red(), None);
        ctx.with_recorder(&mut inner, |ctx| {
            ctx.fill_rect(Rect::new(2.0, 0.0, 2.0, 2.0), Color::green(), None);
        });
        ctx.fill_rect(Rect::new(4.0, 0.0, 2.0, 2.0), Color::blue(), None);
    });

    assert_eq!(outer.len(), 2);
    assert_eq!(inner.len(), 1);
    assert!(ctx.recording_complete());
}

#[test]
fn recorder_scope_clears_target_after_panic() {
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::command::DisplayList;
    use crate::draw::geometry::spatial::Orientation;

    let mut canvas = NoopCanvas2D;
    let fonts = FontService::new();
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        FontHandle::default(),
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        100,
        100,
    );
    let mut aborted = DisplayList::new();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ctx.with_recorder(&mut aborted, |ctx| {
            ctx.fill_rect(Rect::new(0.0, 0.0, 2.0, 2.0), Color::red(), None);
            panic!("abort recorder scope");
        });
    }));
    assert!(result.is_err());
    assert!(!ctx.recording_complete());

    ctx.fill_rect(Rect::new(2.0, 0.0, 2.0, 2.0), Color::green(), None);
    assert_eq!(aborted.len(), 1);

    let mut recovered = DisplayList::new();
    ctx.with_recorder(&mut recovered, |ctx| {
        ctx.fill_rect(Rect::new(4.0, 0.0, 2.0, 2.0), Color::blue(), None);
    });
    assert_eq!(recovered.len(), 1);
    assert!(ctx.recording_complete());
}

#[test]
fn display_list_replay_restores_enabled_recording_state() {
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::command::{DisplayList, PaintOp};
    use crate::draw::geometry::spatial::Orientation;

    let mut canvas = NoopCanvas2D;
    let fonts = FontService::new();
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        FontHandle::default(),
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        100,
        100,
    );
    let mut source = DisplayList::new();
    source.push(PaintOp::FillRect {
        rect: Rect::new(0.0, 0.0, 2.0, 2.0),
        color: Color::red(),
        radius: None,
    });
    let mut captured = DisplayList::new();

    ctx.with_recorder(&mut captured, |ctx| {
        source.replay(ctx);
        ctx.fill_rect(Rect::new(2.0, 0.0, 2.0, 2.0), Color::blue(), None);
    });

    assert_eq!(captured.len(), 1);
    assert!(matches!(captured.ops(), [PaintOp::FillRect { color, .. }] if *color == Color::blue()));
}
