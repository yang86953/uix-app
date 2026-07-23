use crate::draw::backend::contract::BackendCapabilities;
use crate::draw::backend::contract::RenderBackend;
use crate::draw::backend::cpu::{CpuBackend, CpuDrawSurface};
use crate::draw::geometry::path::PathBuilder;
use crate::draw::renderer::lifecycle::*;
use crate::draw::Canvas2D;
use crate::draw::{ScrollCopy, UpdateStrategy};
use crate::tests::common::*;

#[test]
fn normalize_strategy_expands_dirty_to_full_when_no_partial_redraw() {
    let rects = vec![Rect::new(1.0, 2.0, 10.0, 10.0)];
    let normalized = normalize_strategy(
        UpdateStrategy::DirtyRects(rects),
        BackendCapabilities::gpu_full_redraw(),
    );
    assert!(matches!(normalized, UpdateStrategy::FullRedraw));
}

#[test]
fn normalize_strategy_keeps_dirty_when_partial_redraw() {
    let rects = vec![Rect::new(1.0, 2.0, 10.0, 10.0)];
    let normalized = normalize_strategy(
        UpdateStrategy::DirtyRects(rects.clone()),
        BackendCapabilities::cpu(),
    );
    assert!(matches!(normalized, UpdateStrategy::DirtyRects(_)));
}

#[test]
fn normalize_strategy_keeps_dirty_for_gpu_partial() {
    let rects = vec![Rect::new(1.0, 2.0, 10.0, 10.0)];
    let normalized = normalize_strategy(
        UpdateStrategy::DirtyRects(rects.clone()),
        BackendCapabilities::gpu(),
    );
    assert!(matches!(normalized, UpdateStrategy::DirtyRects(_)));
}

#[test]
fn normalize_strategy_expands_scroll_copy_without_memmove_capability() {
    let normalized = normalize_strategy(
        UpdateStrategy::ScrollCopies {
            dirty_rects: vec![Rect::new(0.0, 3.0, 4.0, 1.0)],
            copies: vec![ScrollCopy::new(Rect::new(0.0, 0.0, 4.0, 4.0), 0.0, 1.0)],
        },
        BackendCapabilities::gpu(),
    );
    assert!(matches!(normalized, UpdateStrategy::FullRedraw));
}

#[test]
fn begin_frame_scroll_copy_moves_before_clearing_exposed_strip() {
    let mut surface = CpuDrawSurface::new(4, 4);
    let colors = [
        crate::draw::Color::red(),
        crate::draw::Color::green(),
        crate::draw::Color::blue(),
        crate::draw::Color::from_rgb(255, 255, 0),
    ];
    for (y, color) in colors.into_iter().enumerate() {
        surface
            .canvas_mut()
            .fill_rect(Rect::new(0.0, y as f32, 4.0, 1.0), color, None);
    }

    let dirty = Rect::new(0.0, 3.0, 4.0, 1.0);
    let outcome = begin_frame(
        UpdateStrategy::ScrollCopies {
            dirty_rects: vec![dirty],
            copies: vec![ScrollCopy::new(Rect::new(0.0, 0.0, 4.0, 4.0), 0.0, 1.0)],
        },
        &mut surface,
        4,
        4,
        BackendCapabilities::cpu(),
    );

    assert_eq!(
        outcome,
        RenderOutcome::FrameReady(DamageRegion::partial(vec![dirty]))
    );
    let pixels = surface.surface().pixels();
    assert_eq!(pixels[0], crate::draw::Color::green().premultiplied());
    assert_eq!(pixels[4], crate::draw::Color::blue().premultiplied());
    assert_eq!(
        pixels[8],
        crate::draw::Color::from_rgb(255, 255, 0).premultiplied()
    );
    assert_eq!(
        pixels[12],
        crate::draw::Color::transparent().premultiplied()
    );
    end_frame(&mut surface);
}

#[test]
fn begin_frame_multi_dirty_preserves_disjoint_gap() {
    // 两块不相交 dirty：保留离散矩形，不应把中间空隙提升为 AABB 并清屏
    let mut surface = CpuDrawSurface::new(20, 120);
    // 先铺不透明底色
    surface.canvas_mut().fill_rect(
        Rect::new(0.0, 0.0, 20.0, 120.0),
        crate::draw::Color::from_rgb(255, 0, 0),
        None,
    );
    let _ = begin_frame(
        UpdateStrategy::DirtyRects(vec![
            Rect::new(0.0, 0.0, 20.0, 10.0),
            Rect::new(0.0, 100.0, 20.0, 10.0),
        ]),
        &mut surface,
        20,
        120,
        BackendCapabilities::cpu(),
    );
    let pixels = surface.surface().pixels();
    // 中间 y=50 不属于任一 dirty，必须保留原有像素。
    let mid = (50 * 20 + 5) as usize;
    let red = crate::draw::Color::from_rgb(255, 0, 0).to_rgba();
    assert_eq!(
        pixels[mid], red,
        "disjoint dirty rects must preserve the gap"
    );
    end_frame(&mut surface);
}

#[test]
fn begin_frame_partial_dirty_returns_partial_damage() {
    let rects = vec![Rect::new(1.0, 2.0, 10.0, 10.0)];
    let mut surface = CpuDrawSurface::new(20, 20);
    let outcome = begin_frame(
        UpdateStrategy::DirtyRects(rects.clone()),
        &mut surface,
        20,
        20,
        BackendCapabilities::cpu(),
    );
    assert_eq!(
        outcome,
        RenderOutcome::FrameReady(DamageRegion::partial(rects))
    );
    end_frame(&mut surface);
}

#[test]
fn begin_frame_dirty_clip_intersects_surface_bounds() {
    let mut surface = CpuDrawSurface::new(20, 20);
    let outcome = begin_frame(
        UpdateStrategy::DirtyRects(vec![Rect::new(-10.0, -5.0, 12.0, 8.0)]),
        &mut surface,
        20,
        20,
        BackendCapabilities::cpu(),
    );
    assert!(matches!(outcome, RenderOutcome::FrameReady(_)));
    assert_eq!(
        surface.canvas_mut().current_clip(),
        Rect::new(0.0, 0.0, 2.0, 3.0)
    );
    end_frame(&mut surface);
}

#[test]
fn begin_frame_empty_dirty_returns_idle() {
    let mut surface = CpuDrawSurface::new(10, 10);
    let outcome = begin_frame(
        UpdateStrategy::DirtyRects(vec![]),
        &mut surface,
        10,
        10,
        BackendCapabilities::cpu(),
    );
    assert_eq!(outcome, RenderOutcome::Idle);
}

#[test]
fn begin_frame_full_redraw_returns_ready_target() {
    let mut surface = CpuDrawSurface::new(10, 10);
    let outcome = begin_frame(
        UpdateStrategy::FullRedraw,
        &mut surface,
        10,
        10,
        BackendCapabilities::cpu(),
    );
    assert_eq!(outcome, RenderOutcome::FrameReady(DamageRegion::full()));
    end_frame(&mut surface);
}

#[test]
fn gpu_caps_force_full_clear_on_dirty_rects() {
    let mut surface = CpuDrawSurface::new(20, 20);
    let outcome = begin_frame(
        UpdateStrategy::DirtyRects(vec![Rect::new(0.0, 0.0, 5.0, 5.0)]),
        &mut surface,
        20,
        20,
        BackendCapabilities::gpu_full_redraw(),
    );
    assert_eq!(outcome, RenderOutcome::FrameReady(DamageRegion::full()));
    end_frame(&mut surface);
}

#[test]
fn end_frame_rejects_an_unimplemented_path_clip_instead_of_reporting_present() {
    let mut surface = CpuDrawSurface::new(16, 16);
    let _ = begin_frame(
        UpdateStrategy::FullRedraw,
        &mut surface,
        16,
        16,
        BackendCapabilities::cpu(),
    );
    let mut builder = PathBuilder::new();
    builder
        .move_to(2.0, 2.0)
        .line_to(14.0, 2.0)
        .line_to(8.0, 14.0)
        .close();
    surface.canvas_mut().push_clip_path(&builder.build());
    surface.canvas_mut().fill_rect(
        Rect::new(0.0, 0.0, 16.0, 16.0),
        crate::draw::Color::red(),
        None,
    );

    let outcome = end_frame(&mut surface);
    assert!(matches!(
        outcome,
        RenderOutcome::Failed(crate::draw::renderer::GraphicsFailure::Other(error))
            if error.code() == crate::core::Errc::NotImplemented
    ));
}

#[test]
fn legacy_cpu_present_rejects_a_deferred_path_clip() {
    let mut backend = CpuBackend::new();
    backend.resize(16, 16).expect("resize CPU backend");
    let mut builder = PathBuilder::new();
    builder
        .move_to(2.0, 2.0)
        .line_to(14.0, 2.0)
        .line_to(8.0, 14.0)
        .close();
    backend.surface().canvas().push_clip_path(&builder.build());

    let error = backend
        .present(&DamageRegion::full())
        .expect_err("legacy final present must not hide a deferred canvas failure");
    assert_eq!(error.code(), crate::core::Errc::NotImplemented);
}

#[test]
fn checked_picture_flush_rejects_an_unimplemented_path_clip() {
    let mut backend = CpuBackend::new();
    backend.resize(16, 16).expect("resize CPU backend");
    let handle = backend.create_offscreen(16, 16).expect("Picture target");
    backend
        .try_begin_offscreen_paint(&handle)
        .expect("begin Picture paint");
    let mut builder = PathBuilder::new();
    builder
        .move_to(2.0, 2.0)
        .line_to(14.0, 2.0)
        .line_to(8.0, 14.0)
        .close();
    backend
        .offscreen_canvas(&handle)
        .expect("Picture canvas")
        .push_clip_path(&builder.build());

    let error = backend
        .try_flush_offscreen_paint(&handle)
        .expect_err("unsupported Picture path clip must not flush successfully");
    assert_eq!(error.code(), crate::core::Errc::NotImplemented);
}
