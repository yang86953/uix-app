//! 共享帧逻辑 — clip / clear / Idle 判定，CPU 与 GPU 共用。

use crate::core::Rect;

use crate::draw::backend::traits::{BackendCapabilities, DrawSurface};
use crate::draw::backend::DamageRegion;
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
        UpdateStrategy::DirtyRects(_) => UpdateStrategy::FullRedraw,
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

    let fw = w as f32;
    let fh = h as f32;
    let full = Rect::new(0.0, 0.0, fw, fh);

    match &strategy {
        UpdateStrategy::FullRedraw => {
            surface.push_clip(full);
        }
        UpdateStrategy::DirtyRects(rects) => {
            if rects.is_empty() {
                surface.push_clip(full);
            } else {
                let mut bounds = rects[0];
                for r in &rects[1..] {
                    bounds = bounds.union(r);
                }
                let clip = bounds.intersect(&full).unwrap_or_else(Rect::zero);
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
                // 与 push_clip 一致：清并集 AABB，避免只清离散条带而父背景画满空隙
                let mut bounds = rects[0];
                for r in &rects[1..] {
                    bounds = bounds.union(r);
                }
                let x0 = (bounds.x + 0.5).floor().max(0.0) as i32;
                let y0 = (bounds.y + 0.5).floor().max(0.0) as i32;
                let x1 = (bounds.x + bounds.w + 0.5).floor().max(0.0) as i32;
                let y1 = (bounds.y + bounds.h + 0.5).floor().max(0.0) as i32;
                let cw = (x1 - x0).min(w - x0).max(0);
                let ch = (y1 - y0).min(h - y0).max(0);
                if cw > 0 && ch > 0 {
                    surface.clear_rect_raw(x0, y0, cw, ch);
                }
            }
        }
    }

    RenderOutcome::FrameReady(present_damage_for_strategy(&strategy))
}

/// 帧结束：恢复裁剪栈。
pub fn end_frame(surface: &mut dyn DrawSurface) -> RenderOutcome {
    surface.pop_clip();
    if let Some(error) = surface.take_deferred_error() {
        return RenderOutcome::Failed(crate::draw::engine::GraphicsFailure::from_error(error));
    }
    RenderOutcome::Present(DamageRegion::full())
}

fn present_damage_for_strategy(strategy: &UpdateStrategy) -> DamageRegion {
    match strategy {
        UpdateStrategy::FullRedraw => DamageRegion::full(),
        UpdateStrategy::DirtyRects(rects) => {
            if rects.is_empty() {
                DamageRegion::full()
            } else {
                DamageRegion::partial(rects.clone())
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/draw/pipeline/frame.rs"]
mod tests;
