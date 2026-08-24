//! 共享帧逻辑 — clip / clear / Idle 判定，CPU 与 GPU 共用。

use crate::core::{Point, Rect};

use crate::draw::backend::DamageRegion;
use crate::draw::backend::contract::{BackendCapabilities, DrawSurface};
use crate::draw::outcome::RenderOutcome;
use crate::draw::{ScrollCopy, UpdateStrategy};

/// 根据后端能力规范化更新策略。
///
/// 不支持 `partial_redraw` 的后端自动降级为全帧重绘。
pub fn normalize_strategy(strategy: UpdateStrategy, caps: BackendCapabilities) -> UpdateStrategy {
    match strategy {
        UpdateStrategy::FullRedraw => UpdateStrategy::FullRedraw,
        UpdateStrategy::DirtyRects(rects) if caps.partial_redraw => {
            UpdateStrategy::DirtyRects(rects)
        }
        UpdateStrategy::ScrollCopies {
            dirty_rects,
            copies,
        } if caps.partial_redraw && caps.scroll_memmove => UpdateStrategy::ScrollCopies {
            dirty_rects,
            copies,
        },
        UpdateStrategy::DirtyRects(_) | UpdateStrategy::ScrollCopies { .. } => {
            UpdateStrategy::FullRedraw
        }
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

    if let Some(rects) = strategy.rects() {
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
                // 外层 clip 仍取并集，收紧到全部脏区；逐矩形绘制时再 push 子 clip。
                let mut bounds = rects[0];
                for r in &rects[1..] {
                    bounds = bounds.union(r);
                }
                let clip = bounds.intersect(&full).unwrap_or_else(Rect::zero);
                surface.push_clip(clip);
            }
        }
        UpdateStrategy::ScrollCopies { dirty_rects, .. } => {
            let clip = dirty_bounds(dirty_rects)
                .and_then(|bounds| bounds.intersect(&full))
                .unwrap_or_else(Rect::zero);
            surface.push_clip(clip);
        }
    }

    if let UpdateStrategy::ScrollCopies { copies, .. } = &strategy {
        for copy in copies {
            apply_scroll_copy(surface, *copy);
        }
    }

    if strategy.should_clear() {
        match &strategy {
            UpdateStrategy::FullRedraw => {
                surface.clear_all();
            }
            UpdateStrategy::DirtyRects(rects)
            | UpdateStrategy::ScrollCopies {
                dirty_rects: rects, ..
            } => {
                // 逐矩形清屏：空隙保留旧像素，由各矩形内的父背景重绘填补。
                for bounds in rects {
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
    }

    // 策略到此已完成 clip / clear，直接把脏矩形分配移交给返回值。
    RenderOutcome::FrameReady(present_damage_for_strategy(strategy))
}

/// 帧结束：恢复裁剪栈。
pub fn end_frame(surface: &mut dyn DrawSurface) -> RenderOutcome {
    surface.pop_clip();
    if let Some(error) = surface.take_deferred_error() {
        return RenderOutcome::Failed(crate::draw::renderer::GraphicsFailure::from_error(error));
    }
    RenderOutcome::Present(DamageRegion::full())
}

fn present_damage_for_strategy(strategy: UpdateStrategy) -> DamageRegion {
    match strategy {
        UpdateStrategy::FullRedraw => DamageRegion::full(),
        UpdateStrategy::DirtyRects(rects)
        | UpdateStrategy::ScrollCopies {
            dirty_rects: rects, ..
        } => {
            if rects.is_empty() {
                DamageRegion::full()
            } else {
                DamageRegion::partial(rects)
            }
        }
    }
}

fn dirty_bounds(rects: &[Rect]) -> Option<Rect> {
    let (&first, rest) = rects.split_first()?;
    Some(rest.iter().fold(first, |bounds, rect| bounds.union(rect)))
}

fn apply_scroll_copy(surface: &mut dyn DrawSurface, copy: ScrollCopy) {
    let dx = copy.delta.x.round();
    let dy = copy.delta.y.round();
    if copy.viewport.w <= 0.0
        || copy.viewport.h <= 0.0
        || !dx.is_finite()
        || !dy.is_finite()
        || (dx == 0.0 && dy == 0.0)
    {
        return;
    }
    let source = Rect::new(
        copy.viewport.x + dx,
        copy.viewport.y + dy,
        copy.viewport.w,
        copy.viewport.h,
    );
    surface.copy_region(source, Point::new(copy.viewport.x, copy.viewport.y));
}
