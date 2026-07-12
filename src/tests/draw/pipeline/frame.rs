use super::*;
use crate::draw::backend::cpu::CpuDrawSurface;
use crate::draw::backend::traits::BackendCapabilities;
use crate::draw::traits::Canvas2D;

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
fn begin_frame_multi_dirty_clears_union_aabb() {
    // 两块不相交 dirty：清屏须覆盖并集，避免中间空隙只被父背景盖住
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
    // 中间 y=50 应被并集清屏（默认透明），而非残留红
    let mid = (50 * 20 + 5) as usize;
    let red = crate::draw::Color::from_rgb(255, 0, 0).to_rgba();
    assert_ne!(
        pixels[mid], red,
        "union clear must wipe gap between dirty strips, got {:#x}",
        pixels[mid]
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
