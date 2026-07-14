use crate::draw::painting::paint_context::*;
use crate::draw::spatial::PhysicalUnit;
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
    use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::spatial::Orientation;

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
fn display_list_recording_rejects_untracked_canvas_access() {
    use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::painting::DisplayList;
    use crate::draw::spatial::Orientation;

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
        ctx.fill_rect(Rect::new(0.0, 0.0, 10.0, 10.0), Color::red(), None);
        assert!(ctx.recording_complete());

        let _ = ctx.canvas_2d();
        assert!(!ctx.recording_complete());
    });
}

#[test]
fn display_list_recording_covers_extended_2d_paint_operations() {
    use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::painting::DisplayList;
    use crate::draw::primitives::path::{FillRule, PathBuilder};
    use crate::draw::primitives::stroker::StrokeOptions;
    use crate::draw::spatial::Orientation;

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
    use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::painting::DisplayList;
    use crate::draw::spatial::Orientation;

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
    use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::painting::DisplayList;
    use crate::draw::spatial::Orientation;

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
    use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::painting::DisplayList;
    use crate::draw::spatial::Orientation;

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
