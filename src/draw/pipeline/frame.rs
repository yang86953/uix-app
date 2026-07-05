//! 共享帧逻辑 — clip / clear / Idle 判定，CPU 与 GPU 共用。

use crate::native::Rect;

use crate::draw::backend::DamageRegion;
use crate::draw::backend::traits::{BackendCapabilities, DrawSurface};
use crate::draw::engine::RenderOutcome;
use crate::draw::traits::UpdateStrategy;

/// 根据后端能力规范化更新策略。
///
/// 不支持 `partial_redraw` 的后端自动降级为全帧重绘。
pub fn normalize_strategy(strategy: UpdateStrategy, caps: BackendCapabilities) -> UpdateStrategy {
    if caps.partial_redraw {
        return strategy;
    }
    match strategy {
        UpdateStrategy::FullRedraw => UpdateStrategy::FullRedraw,
        UpdateStrategy::DirtyRects(_) | UpdateStrategy::Overlay(_) => UpdateStrategy::FullRedraw,
    }
}

/// 帧开始：设置裁剪并清除脏区域。
pub fn begin_frame(
    strategy: UpdateStrategy,
    surface: &mut dyn DrawSurface,
    width: i32,
    height: i32,
    caps: BackendCapabilities,
) -> RenderOutcome {
    let strategy = normalize_strategy(strategy, caps);

    if let UpdateStrategy::DirtyRects(rects) = &strategy {
        if rects.is_empty() {
            return RenderOutcome::Idle;
        }
    }

    let w = width;
    let h = height;

    if !strategy.should_clear() {
        if let UpdateStrategy::Overlay(rects) = &strategy {
            if rects.is_empty() {
                return RenderOutcome::Idle;
            }
        }
    }

    let fw = w as f32;
    let fh = h as f32;
    let full = Rect::new(0.0, 0.0, fw, fh);

    match &strategy {
        UpdateStrategy::FullRedraw => {
            surface.push_clip(full);
        }
        UpdateStrategy::DirtyRects(rects) | UpdateStrategy::Overlay(rects) => {
            if rects.is_empty() {
                surface.push_clip(full);
            } else {
                let mut bounds = rects[0];
                for r in &rects[1..] {
                    bounds = bounds.union(r);
                }
                let clip = Rect::new(
                    bounds.x.max(0.0),
                    bounds.y.max(0.0),
                    bounds.w.min(fw - bounds.x.max(0.0)),
                    bounds.h.min(fh - bounds.y.max(0.0)),
                );
                surface.push_clip(clip);
            }
        }
    }

    if strategy.should_clear() {
        match &strategy {
            UpdateStrategy::FullRedraw => {
                surface.clear_all();
            }
            UpdateStrategy::DirtyRects(rects) => {
                for r in rects {
                    let x0 = (r.x + 0.5).floor().max(0.0) as i32;
                    let y0 = (r.y + 0.5).floor().max(0.0) as i32;
                    let x1 = (r.x + r.w + 0.5).floor().max(0.0) as i32;
                    let y1 = (r.y + r.h + 0.5).floor().max(0.0) as i32;
                    let cw = (x1 - x0).min(w - x0).max(0);
                    let ch = (y1 - y0).min(h - y0).max(0);
                    if cw > 0 && ch > 0 {
                        surface.clear_rect_raw(x0, y0, cw, ch);
                    }
                }
            }
            UpdateStrategy::Overlay(_) => {}
        }
    }

    RenderOutcome::Present(present_damage_for_strategy(&strategy))
}

/// 帧结束：恢复裁剪栈。
pub fn end_frame(surface: &mut dyn DrawSurface) -> RenderOutcome {
    surface.pop_clip();
    RenderOutcome::Present(DamageRegion::full())
}

fn present_damage_for_strategy(strategy: &UpdateStrategy) -> DamageRegion {
    match strategy {
        UpdateStrategy::FullRedraw => DamageRegion::full(),
        UpdateStrategy::DirtyRects(rects) | UpdateStrategy::Overlay(rects) => {
            if rects.is_empty() {
                DamageRegion::full()
            } else {
                DamageRegion::partial(rects.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::backend::cpu::CpuDrawSurface;
    use crate::draw::backend::traits::BackendCapabilities;

    #[test]
    fn normalize_strategy_expands_dirty_to_full_when_no_partial_redraw() {
        let rects = vec![Rect::new(1.0, 2.0, 10.0, 10.0)];
        let normalized =
            normalize_strategy(UpdateStrategy::DirtyRects(rects), BackendCapabilities::gpu_full_redraw());
        assert!(matches!(normalized, UpdateStrategy::FullRedraw));
    }

    #[test]
    fn normalize_strategy_keeps_dirty_when_partial_redraw() {
        let rects = vec![Rect::new(1.0, 2.0, 10.0, 10.0)];
        let normalized =
            normalize_strategy(UpdateStrategy::DirtyRects(rects.clone()), BackendCapabilities::cpu());
        assert!(matches!(normalized, UpdateStrategy::DirtyRects(_)));
    }

    #[test]
    fn normalize_strategy_keeps_dirty_for_gpu_partial() {
        let rects = vec![Rect::new(1.0, 2.0, 10.0, 10.0)];
        let normalized =
            normalize_strategy(UpdateStrategy::DirtyRects(rects.clone()), BackendCapabilities::gpu());
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
        assert_eq!(outcome, RenderOutcome::Present(DamageRegion::partial(rects)));
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
}
