use super::*;
use crate::draw::backend::cpu::CpuDrawSurface;
use crate::draw::backend::traits::BackendCapabilities;

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
        RenderOutcome::Present(DamageRegion::partial(rects))
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
fn begin_frame_full_redraw_presents() {
    let mut surface = CpuDrawSurface::new(10, 10);
    let outcome = begin_frame(
        UpdateStrategy::FullRedraw,
        &mut surface,
        10,
        10,
        BackendCapabilities::cpu(),
    );
    assert_eq!(outcome, RenderOutcome::Present(DamageRegion::full()));
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
    assert_eq!(outcome, RenderOutcome::Present(DamageRegion::full()));
    end_frame(&mut surface);
}
